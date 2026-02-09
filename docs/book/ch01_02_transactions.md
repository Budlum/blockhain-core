# Bölüm 1.2: İşlemler ve Veri Transferi Mimarisi

Bu bölüm, blok zincirindeki değer transferinin (Value Transfer) nasıl gerçekleştiğini, `Transaction` yapısının her bir parçasını neden koyduğumuzu ve güvenliği nasıl sağladığımızı anlatır.

Kaynak Dosya: `src/transaction.rs`

---

## 1. Veri Yapıları: Bir İşlemin Anatomisi

Bir işlem (`Transaction`), "A kişisi B kişisine X miktar para gönderdi" cümlesinin dijital ve kriptografik halidir.

### Enum: `TransactionType`

İşlemler sadece para göndermek için değildir. Sistemin yönetimi için de kullanılırlar.

**Kod:**
```rust
pub enum TransactionType {
    Transfer, // Standart: Alice -> Bob (10 Coin)
    Stake,    // Validatör Olma: Paramı kilitliyorum ve ağa hizmet edeceğim.
    Unstake,  // Çıkış: Paramı çöz ve faiziyle geri ver.
    Vote,     // Yönetişim: Ağın parametrelerini değiştirmek için oy veriyorum.
}
```

**Neden Var?**
Eğer bu Tipler olmasaydı, Stake etmek için "Burn Adresi"ne para yollamak gibi dolambaçlı yollar (workaround) kullanmak zorunda kalırdık. İşlem tipini açıkça (`explicit`) belirtmek, kodun okunabilirliğini ve güvenliğini artırır. `AccountState` bu tipe bakarak ne yapacağına karar verir.

---

### Struct: `Transaction`

**Kod:**
```rust
pub struct Transaction {
    pub from: String,       // Gönderen (Public Key)
    pub to: String,         // Alıcı (Public Key)
    pub amount: u64,        // Miktar
    pub fee: u64,           // İşlem Ücreti (Gas Fee)
    pub nonce: u64,         // Sıra Numarası (Anti-Replay)
    pub data: Vec<u8>,      // Ek Veri (Memo / Smart Contract Call)
    pub timestamp: u128,    // Zaman
    pub hash: String,       // İşlem ID (TxID)
    pub signature: Option<Vec<u8>>, // Dijital İmza
    pub chain_id: u64,      // Ağ ID (Chain Isolation)
    pub tx_type: TransactionType, // Tip
}
```

**Satır Satır Analiz:**

| Alan Adı | Veri Tipi | Neden Bu Tipi Seçtik? | Ne İşe Yarar & Neden Gerekli? |
| :--- | :--- | :--- | :--- |
| `from` | `String` | 64-char Hex String. | **Gönderen.** Kimin bakiyesinden düşülecek? Aynı zamanda imza doğrulamasında kullanılan Public Key'dir. |
| `to` | `String` | Hex String. | **Alıcı.** Para kime gidecek? Stake işlemlerinde boş olabilir (kendine stake). |
| `amount` | `u64` | `u64` | **Miktar.** Transfer edilecek değer. Kuruş (decimal) sorunlarıyla uğraşmamak için genellikle en küçük birim (Raw/Wei gibi) cinsinden tutulur. |
| `fee` | `u64` | `u64` | **Rüşvet / Ücret.** Madencilerin/Validatörlerin bu işlemi bloğa koyması için ödenen teşviktir. Aynı zamanda Spam saldırılarını (milyonlarca bedava işlem) engeller. |
| `nonce` | `u64` | Sıralı sayı. | **Anti-Replay Sayacı.** EN KRİTİK ALANLARDAN BİRİ. Alice Bob'a 10 coin yolladı. Bob bu işlemi ağa tekrar tekrar "replay" edip Alice'i soymasın diye var. Bir nonce sadece **BİR KERE** kullanılır. |
| `data` | `Vec<u8>`| Byte dizisi. | **Memo / Veri.** "Kira ödemesi" gibi notlar veya ileride Smart Contract çağrıları için veri alanı. |
| `signature`| `Option<Vec>` | Opsiyonel Byte dizisi. | **İmza.** "Bu işlemi gerçekten `from` adresinin sahibi mi yaptı?" sorusunun cevabı. Özel anahtar (Private Key) ile atılır. |
| `chain_id`| `u64` | Sabit Sayı. | **Zincir İzolasyonu.** Budlum Mainnet için üretilen bir imzalı işlemin, Budlum Testnet'te geçerli olmasını (veya tam tersi) engeller. |

---

## 2. Algoritmalar: Güvenlik Nasıl Sağlanır?

### Fonksiyon: `signing_hash` (İmzalanacak Veri)

Bir evrağı imzalamadan önce, neyi imzaladığınızı sabitlemeniz gerekir. Bu fonksiyon, işlemin "özünü" çıkarır.

```rust
pub fn signing_hash(&self) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    // 1. Domain Separation Tag: Karışıklığı önle.
    hasher.update(b"BDLM_TX_V2"); 
    
    // 2. Kritik alanları ekle.
    hasher.update(self.from.as_bytes());
    hasher.update(self.to.as_bytes());
    hasher.update(self.amount.to_le_bytes());
    hasher.update(self.fee.to_le_bytes());
    hasher.update(self.nonce.to_le_bytes()); // <--- Nonce burada çok önemli!
    hasher.update(&self.data);
    hasher.update(self.chain_id.to_le_bytes()); // <--- Chain ID burada!
    
    // 3. İmzayı EKLEME! (signature hariç her şeyi hashle)
    hasher.finalize().into()
}
```

**Soru:** `hash` ve `signature` alanlarını neden eklemedik?
**Cevap:**
-   `signature`: İmzayı hesaplamak için bu hash lazım. Hash'in içine imzayı koyamazsınız (Tavuk-Yumurta problemi).
-   `hash`: İşlemin ID'si (TxID) genellikle tüm verinin (imza dahil) hash'idir. İmzalamadan önce ID belli olmayabilir.

**Tasarım Notu:** `nonce` ve `chain_id` alanlarını bu hash'e dahil etmek ZORUNLUDUR. Yoksa Replay Attack (Tekrar Saldırısı) yapılabilir.
-   Chain ID dahil olmasaydı: Testnet'teki işlem Mainnet'te de geçerli olurdu.
-   Nonce dahil olmasaydı: Geçmişteki bir para transferi tekrar tekrar gönderilip bakiye boşaltılırdı.

---

### Fonksiyon: `check_validity` (Mantıksal Doğrulama)

Kriptografik doğrulama (`verify`) yetmez, bir de iş mantığı (`business logic`) kontrolü gerekir.

**Kodda `is_valid` Fonksiyonu:**

```rust
pub fn is_valid(&self) -> bool {
    // 1. Önce imzayı kontrol et. İmza yoksa diğerlerine bakmaya gerek bile yok.
    if !self.verify() { return false; }

    // 2. İşlem tipine göre özel kontroller yap.
    match self.tx_type {
        TransactionType::Transfer => {
            // Transferde alıcı adresi BOŞ OLAMAZ. Parayı uzaya gönderemeyiz.
            if self.to.is_empty() { return false; }
        }
        TransactionType::Stake => {
            // Stake miktarı 0 OLAMAZ. Sisteme yük bindirir.
            if self.amount == 0 { return false; }
        }
        // ...
    }
    true
}
```

**Neden Bunu Yazdık?**
Ağı "çöp" veriden korumak için. İmzası geçerli olsa bile, alıcısı olmayan bir transfer işlemi veritabanında gereksiz yer kaplar. Bu fonksiyon, bu tür mantıksız işlemleri daha havuza (Mempool) girmeden reddetmemizi sağlar.

---

### Fonksiyon: `sign` (İmzalama Süreci)

Bu fonksiyon client (cüzdan) tarafında çalışır.

```rust
pub fn sign(&mut self, keypair: &KeyPair) {
    // 1. Güvenlik Kontrolü: Yanlış anahtarla mı imzalamaya çalışıyoruz?
    // İşlemdeki 'from' adresi, elimizdeki Private Key'e ait mi?
    if self.from != keypair.public_key_hex() {
        println!("HATA: Başkasının adına imza atamazsın!");
    }

    // 2. İmzalanacak özeti çıkar.
    let hash = self.signing_hash();
    
    // 3. Kriptografik imza üret (Ed25519).
    let signature = keypair.sign(&hash);
    
    // 4. İmzayı işlem nesnesine yapıştır.
    self.signature = Some(signature);
}
```

**Benzerlik:** Islak imza atmak gibidir. Önce metni yazarsınız (`signing_hash`), sonra altına imza atarsınız (`sign`). Metin değişirse imza geçersiz olur.
