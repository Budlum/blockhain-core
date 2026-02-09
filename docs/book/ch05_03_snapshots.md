# Bölüm 5.3: Snapshot ve Veri Budama (Pruning)

Bu bölüm, blok zincirinin sonsuza kadar büyümesini engelleyen **Pruning (Budama)** mekanizmasını ve ağa yeni katılanların günlerce beklemeden senkronize olmasını sağlayan **Snapshot (Anlık Görüntü)** stratejisini analiz eder.

Kaynak Dosya: `src/snapshot.rs` (Varsayımsal)

---

## 1. Problem: Zincir Şişkinliği (State Bloat)

Bir blok zinciri 10 yıl çalışırsa ne olur?
-   **Bitcoin:** 500 GB+ veri.
-   **Ethereum:** 1 TB+ veri (Arşiv Node).

Her düğümün bu kadar veriyi saklaması pahalıdır. Disk dolar, indeksleme yavaşlar.
Çözüm: **Sadece son durumu sakla, geçmişi sil.**

---

## 2. Veri Yapıları: Anlık Görüntü

Snapshot, bir video kaydının (Blockchain) tek bir karesini (State) alıp JPG olarak saklamak gibidir.

### Struct: `Snapshot`

```rust
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub height: u64,             // Hangi blokta çekildi? (Örn: 100.000)
    pub state_root: String,      // O anki hesap durumunun özeti
    pub accounts: Vec<Account>,  // Tüm hesap bakiyeleri
    pub validators: Vec<Validator>, // O anki validatör listesi
}
```

**Analiz:**
Bu yapı, `AccountState`'in serileştirilmiş (paketlemiş) halidir. İçinde geçmiş işlemler (`Transaction`) veya eski bloklar (`Block`) YOKTUR. Sadece "Şu an kimin ne kadar parası var?" bilgisi vardır.

---

## 3. Algoritmalar: Budama Mantığı

### Fonksiyon: `create_snapshot` (Fotoğraf Çekme)

Belirli aralıklarla (örneğin her 10.000 blokta bir / Epoch sonu) çalışır.

```rust
pub fn create_snapshot(&self, state: &AccountState) {
    if state.epoch_index % SNAPSHOT_INTERVAL == 0 {
        let snapshot = Snapshot {
            height: state.epoch_index,
            accounts: state.accounts.values().cloned().collect(),
            // ...
        };
        
        // Diske kaydet: "snapshot_100000.bin"
        save_to_disk(snapshot);
    }
}
```

### Fonksiyon: `prune_history` (Geçmişi Silme)

Eğer elimizde 100.000. bloğun snapshot'ı varsa, 0 ile 90.000 arasındaki bloklara artık ihtiyacımız yoktur (Validasyon için).

```rust
pub fn prune_history(&self, current_height: u64) {
    // Güvenlik Marjı: Son 1000 bloğu silme (Reorg olabilir).
    let safe_height = current_height - 1000;
    
    // Veritabanını tara.
    for i in 0..safe_height {
        self.db.remove(format!("BLOCK:{}", i));
        self.db.remove(format!("TX:{}", i)); // O bloktaki işlemleri de sil.
    }
    
    // Veritabanını sıkıştır (Compact/Vacuum).
    self.db.compact();
}
```

**Neden Güvenlik Marjı (Safety Margin)?**
Zincirin en ucunda bazen çatallanmalar (Micro-forks) olur. Son 10-20 blok değişebilir. Eğer snapshot alır almaz hemen önceki blokları silersek ve zincir başka bir dala (Reorg) geçerse, verisiz kalırız ve düğüm çöker. Bu yüzden her zaman bir miktar "tampon bölge" (buffer) bırakılır.

---

## 4. State Sync (Hızlı Senkronizasyon)

Yeni bir düğüm kurduğunuzu düşünün.
-   **Full Sync (Yavaş):** Genesis'ten başla. 1 Milyon bloğu tek tek indir, işlemleri çalıştır (`apply_transaction`). Aylar sürebilir.
-   **State Sync (Hızlı):**
    1.  Arkadaşına sor: "En son snapshot kaç?" -> "1.000.000"
    2.  Snapshot dosyasını indir (500 MB).
    3.  `AccountState`'i bu dosyadan yükle.
    4.  Sadece 1.000.001'den itibaren blokları indirmeye başla.
    5.  Süre: 10 Dakika.

Kullanıcılarımız için **State Sync** varsayılan yöntem olmalıdır.

---

## Özet

`src/snapshot.rs` ve Budama mekanizması sayesinde:
1.  **Disk Tasarrufu:** TB'larca gereksiz veri saklanmaz.
2.  **Hız:** Yeni düğümler ağa dakikalar içinde katılır.
3.  **Sürdürülebilirlik:** Blok zinciri sonsuza kadar çalışabilir.
