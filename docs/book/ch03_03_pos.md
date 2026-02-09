# Bölüm 3.3: Proof of Stake (PoS) Motoru

Bu bölüm, modern blok zincirlerinin tercihi olan PoS (Hisse Kanıtı) algoritmasını; lider seçim matematiğini, ceza (slashing) sistemini ve konsensüs güvenliğini satır satır inceler.

Kaynak Dosya: `src/consensus/pos.rs`

---

## 1. Veri Yapıları: Oyunun Kuralları

PoS, parası olanın söz sahibi olduğu, ancak hata yapanın parasını kaybettiği bir ekonomik oyundur.

### Struct: `PoSConfig`

Sistem parametreleri.

**Kod:**
```rust
pub struct PoSConfig {
    pub min_stake: u64,          // Min. Teminat (örn. 32 ETH)
    pub slot_duration: u64,      // Her blok kaç saniye? (12 sn)
    pub epoch_length: u64,       // Bir devir kaç blok sürer? (32 blok)
    pub slashing_penalty: f64,   // Suçun bedeli (Örn. %10)
}
```

**Analiz:**

| Alan Adı | Veri Tipi | Neden? | Açıklama |
| :--- | :--- | :--- | :--- |
| `min_stake` | `u64` | `u64` | **Giriş Barajı.** Herkesin validatör olmasını engeller. Çok fazla küçük validatör, ağ trafiğini şişirir. Ciddi oyuncuları seçmek için bir eşik vardır. |
| `slot_duration` | `u64` | `u64` | **Zaman Dilimi.** PoW'da blok süresi rastgeledir (Bulunca biter). PoS'ta ise zaman **Slot**lara bölünmüştür (Tık-tak saat gibi). Her slotta sadece bir kişi blok üretebilir. |
| `epoch_length` | `u64` | `32` | **Devir.** Belirli periyotlarda yönetimsel işlemler yapılır (Ödül dağıtımı, Validatör setinin değişmesi, Checkpoint alınması). |

---

### Struct: `PoSEngine`

**Kod:**
```rust
pub struct PoSEngine {
    config: PoSConfig,
    seen_blocks: RwLock<HashMap<(String, u64), String>>, // Çift imza yakalamak için
    slashing_evidence: RwLock<Vec<SlashingEvidence>>,    // Tespit edilen suçlar
    keypair: Option<KeyPair>,                            // Eğer biz validatörsek
}
```

**Thread Safety (`RwLock`):**
PoS motoru aynı anda hem blok üretebilir (Mining Thread) hem de ağdan gelen blokları dinleyebilir (Network Thread). Bu yüzden paylaşılan verilere erişim `RwLock` (Okuma-Yazma Kilidi) ile korunur.

---

## 2. Algoritmalar: Seçim ve Ceza

### Fonksiyon: `select_validator` (Lider Kim?)

Her slot için kimin blok üreteceğini belirleyen "Kura Çekimi" fonksiyonudur.

```rust
pub fn select_validator(&self, state: &AccountState, previous_hash: &str, slot: u64) -> Option<String> {
    // 1. Şans Tohumu (Seed) oluştur: Önceki blok hash'i + Slot Numarası.
    // Bu değer herkes için aynıdır (Deterministik).
    let seed_input = format!("{}{}", previous_hash, slot);
    let seed_hash = hash(seed_input); 
    
    // 2. Hash'i büyük bir sayıya çevir (u128).
    let seed_num = u128::from_le_bytes(seed_hash[0..16].try_into().unwrap());

    // 3. Toplam hisseyi (Total Stake) hesapla.
    let total_stake: u64 = state.validators.values()
        .filter(|v| v.active)
        .map(|v| v.stake)
        .sum();

    if total_stake == 0 { return None; }

    // 4. Kazanan noktayı belirle: `Seed % TotalStake`
    // Bu, 0 ile TotalStake-1 arasında bir sayıdır.
    let mut target = (seed_num % total_stake as u128) as u64;

    // 5. Validatörleri gez ve "target" kimin hisse aralığına düşüyor bul.
    // (Weighted Random Selection)
    for (address, validator) in &state.validators {
        if !validator.active { continue; }
        
        if target < validator.stake {
            return Some(address.clone()); // Kazanan sensin!
        }
        target -= validator.stake;
    }
    None
}
```

**Soru:** Neden `previous_hash` kullanıyoruz?
**Cevap:** Eğer sadece `slot` numarasına göre seçseydik, liderler 100 yıl boyunca önceden belli olurdu. Saldırganlar "Seneye Salı günü liderim" diyerek o günü bekleyip saldırı yapabilirdi. `previous_hash` (önceki blok), sürekli değişen bir rastgelelik kaynağıdır.

---

### Fonksiyon: `record_block` (Dedektiflik)

Ağa gelen her bloğu kaydeder ve "Double Signing" arar.

```rust
pub fn record_block(&self, block: &Block) {
    let producer = block.producer.as_ref().unwrap();
    let index = block.index;
    let hash = &block.hash;

    // Hafıza kilidini al (Yazma modu).
    let mut seen = self.seen_blocks.write().unwrap();
    let key = (producer.clone(), index);

    // Eğer bu validatör, bu index (yükseklik) için daha önce blok göndermişse...
    if let Some(existing_hash) = seen.get(&key) {
        if existing_hash != hash {
            // ...ve hash'i farklıysa (Yani içeriği farklı iki blok üretmişse).
            println!("🚨 SUÇ TESPİT EDİLDİ! Validatör: {}", producer);
            
            // Kanıt oluştur ve havuza at.
            self.slashing_evidence.write().unwrap().push(SlashingEvidence { ... });
        }
    } else {
        // İlk kez görüyoruz, kaydet.
        seen.insert(key, hash.clone());
    }
}
```

**Bu Algoritma Neyi Çözer?**
"Nothing at Stake" problemini çözer. Eğer bir validatör, zincir çatallandığında (fork) "her iki tarafa da oynayayım" derse, bu fonksiyon onu yakalar. İki farklı hash'e sahip aynı indexli blok, suçun tartışılmaz kanıtıdır.

---

### Fonksiyon: `prepare_block` (Blok Üretimi)

Eğer sıra bizdeyse çalışır.

```rust
fn prepare_block(&self, block: &mut Block, state: &AccountState) {
    // 1. Önce bekleyen "Suç Kanıtları"nı bloğa ekle.
    // Adalet gecikmemeli.
    {
        let mut evidence_pool = self.slashing_evidence.write().unwrap();
        if !evidence_pool.is_empty() {
            block.header.slashing_evidence = Some(evidence_pool.clone());
            evidence_pool.clear(); // Blok içine aldık, havuzdan sil.
        }
    }

    // 2. İmza At.
    if let Some(keypair) = &self.keypair {
        block.sign(keypair);
    }
}
```

**Tasarım Notu:**
Ceza kanıtlarını (`slashing_evidence`) bloğun içine koyuyoruz. Çünkü tüm ağın, o validatörün neden cezalandırıldığını (neden bakiyesinin silindiğini) görmesi ve doğrulaması gerekir. Blok zinciri şeffaf bir mahkemedir.

---

## Özet

`src/consensus/pos.rs`, bir yazılım kodundan ziyade bir "Anayasa" gibidir.
-   **Seçim Kanunu:** `select_validator` ile kimin yöneteceği belirlenir.
-   **Ceza Kanunu:** `record_block` ve `SlashingEvidence` ile kurallara uymayanlar cezalandırılır.
-   **Yürütme:** `prepare_block` ile kararlar uygulanır (blok üretilir).
