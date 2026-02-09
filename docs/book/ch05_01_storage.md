# Bölüm 5.1: Kalıcı Depolama (Sled DB)

Bu bölüm, verilerin RAM'den diske nasıl aktarıldığını, `sled` veritabanı yapısını ve "Key-Value" tasarımını analiz eder.

Kaynak Dosya: `src/storage.rs`

---

## 1. Veri Yapıları: Sled Nedir?

Budlum, `SQL` (Tablolar) yerine `NoSQL` (Key-Value) kullanır. Gömülü (Embedded) bir veritabanı olan **Sled** seçilmiştir.

### Neden Sled?
1.  **Gömülü:** Kurulum gerektirmez (PostgreSQL kurulumu gerekmez). Kodun içindedir. Programla birlikte derlenir.
2.  **Hızlı:** Modern NVMe diskler için optimize edilmiştir.
3.  **Thread-Safe:** Aynı anda birçok thread okuma/yazma yapabilir.

### Struct: `Storage`

```rust
#[derive(Clone)] // Clone ucuzdur, sadece dosya tanıtıcısını kopyalar.
pub struct Storage {
    db: Db, // Sled veritabanı handle'ı
}
```

---

## 2. Şema Tasarımı (Schema Design)

Veritabanında tablolar yoktur, sadece Anahtarlar (Key) ve Değerler (Value) vardır. Düzen sağlamak için **Prefix (Önek)** kullanırız.

| Veri Tipi | Anahtar Formatı (Key) | Değer (Value) | Açıklama |
| :--- | :--- | :--- | :--- |
| **Blok** | `BLOCK:{Hash}` | `Serialized(Block)` | Blok verilerini hash ile saklarız. |
| **İşlem** | `TX:{Hash}` | `Serialized(Transaction)` | İşlemleri hash ile saklarız. |
| **Son Blok**| `LAST_BLOCK` | `Hash` (String) | Zincirin en ucunu (Tip) gösteren işaretçidir. |

---

## 3. Kod Analizi

### Fonksiyon: `insert_block`

```rust
pub fn insert_block(&self, block: &Block) -> io::Result<()> {
    // 1. Bloğu Bincode değil JSON yapıyoruz (Debug kolaylığı için, opsiyonel).
    // Gerçek mainnet'te Bincode olmalı.
    let serialized = serde_json::to_vec(block)?;

    // 2. Anahtarı oluştur: BLOCK + Hash
    let key = format!("BLOCK:{}", block.hash);

    // 3. Veritabanına yaz. (Bellek tamponuna yazar)
    self.db.insert(key, serialized)?;

    // 4. KRİTİK ADIM: Flush
    // Veriyi diske fiziksel olarak yaz. Elektrik kesilirse kaybolmasın.
    self.db.flush()?;

    Ok(())
}
```

### Fonksiyon: `load_chain` (Başlangıç Yüklemesi)

Program açıldığında zinciri disken okur.

```rust
pub fn load_chain(&self) -> Vec<Block> {
    let mut chain = Vec::new();

    // 1. En son nerede kaldığımızı öğren.
    // "LAST_BLOCK" anahtarına bak.
    if let Some(last_hash_bytes) = self.db.get("LAST_BLOCK").unwrap() {
        let mut current_hash = String::from_utf8(last_hash_bytes.to_vec()).unwrap();

        // 2. Geriye doğru (Backtracking) yürü.
        loop {
            // Hash ile bloğu getir.
            let block = self.get_block(&current_hash).unwrap();
            
            // Önceki hash'i kaydet.
            let prev_hash = block.previous_hash.clone();
            
            // Zincire ekle.
            chain.push(block);

            // Eğer Genesis ise (Hash=000...) dur.
            if prev_hash == "0".repeat(64) {
                break;
            }
            current_hash = prev_hash;
        }
    }
    
    // 3. Tersten geldiğimiz için listeyi düzelt.
    chain.reverse();
    chain
}
```

**Tasarım Notu:**
Blockchain, aslında bir **Linked List** (Bağlı Liste) veri yapısıdır. Veritabanında her eleman bir öncekini işaret eder. Bu fonksiyon, bu bağlı listeyi takip ederek bütün zinciri yeniden inşa eder.
