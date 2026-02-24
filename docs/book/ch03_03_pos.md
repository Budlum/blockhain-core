# Bölüm 3.3: Proof of Stake (PoS) Motoru ve RANDAO

Bu bölüm, modern blok zincirlerinin tercihi olan PoS (Hisse Kanıtı) algoritmasını; **RANDAO stili rastlantısal (unbiased) lider seçim matematiğini**, ceza (slashing) sistemini ve konsensüs güvenliğini satır satır inceler.

Kaynak Dosya: `src/consensus/pos.rs`

---

## 1. Veri Yapıları: Oyunun Kuralları

PoS, parası olanın söz sahibi olduğu, ancak hata yapanın parasını kaybettiği bir ekonomik oyundur.

### Struct: `PoSConfig` ve `PoSEngine`

```rust
pub struct PoSConfig {
    pub min_stake: u64,          // Min. Teminat (örn. 32 ETH)
    pub slot_duration: u64,      // Her blok kaç saniye? (12 sn)
    pub epoch_length: u64,       // Bir devir kaç blok sürer? (32 blok)
    pub slashing_penalty: f64,   // Suçun bedeli (Örn. %10)
}

pub struct PoSEngine {
    config: PoSConfig,
    seen_blocks: RwLock<HashMap<(String, u64), String>>, // Çift imza yakalamak için
    slashing_evidence: RwLock<Vec<SlashingEvidence>>,    // Tespit edilen suçlar
    epoch_seed: RwLock<[u8; 32]>,                        // RANDAO Lider Seçim Tohumu
    keypair: Option<KeyPair>,                            // Eğer biz validatörsek
}
```

**Analiz:**
- **`epoch_seed` (RANDAO Ortak Tohumu):** Ağdaki rastgelelik (randomness) kaynağıdır. `RwLock` ile korunur. Eski tasarımdaki tekil blok bağımlılığını (ve manipülasyonları) çözer.

---

## 2. Algoritmalar: RANDAO Lider Seçimi ve Ceza

### Fonksiyon: `select_validator` (Lider Kim?)

Her slot için kimin blok üreteceğini belirleyen "Kura Çekimi" fonksiyonudur. Eski ve manipüle edilebilir yaklaşım (`previous_hash` kullanımı) **Mainnet Hardening** işlemi ile RANDAO stiline güncellendi.

```rust
pub fn select_validator(&self, state: &AccountState, _previous_hash: &str, slot: u64) -> Option<String> {
    let total_stake = state.get_total_stake();
    if total_stake == 0 { return None; }

    // 1. RANDAO Tohumunu Al
    let seed = self.epoch_seed.read().unwrap();
    
    // 2. SHA3(Epoch_Seed || Slot)
    let mut hasher = Sha3_256::new();
    hasher.update(*seed);
    hasher.update(slot.to_le_bytes());
    let hash = hasher.finalize();

    let random_value = u64::from_le_bytes(hash[0..8]...);
    let selection_point = random_value % total_stake;

    // 3. Kazananı Bul (Weighted Selection)
    let mut cumulative: u64 = 0;
    for validator in state.get_active_validators() {
        cumulative += validator.effective_stake();
        if selection_point < cumulative {
            return Some(validator.address.clone());
        }
    }
    None
}
```

### Fonksiyon: `record_block` (Seed Toplama & Dedektiflik)

Ağa gelen her bloğu kaydeder. İki önemli görevi vardır: **Çift imza yakalamak** ve **RANDAO Tohumunu Güncellemek**.

```rust
pub fn record_block(&self, block: &Block) {
    // 1. RANDAO Tohumu Güncellemesi (XOR-Mix)
    let block_hash_bytes = hex::decode(&block.hash).unwrap();
    let mut block_contrib = Sha3_256::new();
    block_contrib.update(&block_hash_bytes);
    let contribution: [u8; 32] = block_contrib.finalize().into();

    if let Ok(mut seed) = self.epoch_seed.write() {
        // Her blok, epoch_seed'i XOR ile mutasyona uğratır.
        for (i, byte) in seed.iter_mut().enumerate() {
            *byte ^= contribution[i];
        }
    }
    
    // 2. Double-Sign Tespiti
    // ... Eğer aynı index için farklı hash atanmışsa SlashingEvidence oluştur.
}
```

**Neden RANDAO (XOR-Mix)?**
Eski yapıda `previous_hash` kullanılıyordu. Bir düğüm çıkaracağı bloğu manipüle edip ufak TX değişiklikleri ile hash'i değiştirerek "sıradaki bloğu da" kendine düşürebilirdi. 
RANDAO ile, tüm blokların hash'leri ardışık olarak (`XOR` işlemi) birbirine karıştırılır. Epoch bitene kadar hiçkimse tam teşekküllü Epoch Tohumu'nun ne olacağını %100 kestiremez ve oyun oynayamaz (Bias-Resistance).

---

### Fonksiyon: `prepare_block` (Blok Üretimi)

Eğer sıra bizdeyse çalışır.

```rust
fn prepare_block(&self, block: &mut Block, state: &AccountState) {
    // 1. Önce bekleyen "Suç Kanıtları"nı bloğa ekle. Adalet gecikmemeli.
    {
        let mut evidence_pool = self.slashing_evidence.write().unwrap();
        if !evidence_pool.is_empty() {
            block.slashing_evidence = Some(evidence_pool.clone());
            evidence_pool.clear(); 
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
-   **Seçim Kanunu:** `select_validator` ve `epoch_seed` ile RANDAO rastgeleliğinde kimin yöneteceğini belirler.
-   **Ceza Kanunu:** `record_block` ve `SlashingEvidence` ile kurallara uymayanlar cezalandırılır.
-   **Yürütme:** `prepare_block` ile kararlar uygulanır (blok üretilir).
