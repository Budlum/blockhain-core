# Bölüm 5.2: İşlem Havuzu (Mempool) Mekaniği

Bu bölüm, ağa gelen işlemlerin bloklara girmeden önce beklediği "Bekleme Odası" olan Mempool'u, ücret piyasasını (Fee Market) ve sıralama algoritmalarını analiz eder.

Kaynak Dosya: `src/mempool.rs`
---

## 1. Veri Yapıları: Çoklu Sıralama

Mempool, basit bir liste değildir. İşlemleri farklı kriterlere göre hızlıca bulabilmemiz gerekir.

### Struct: `Mempool`

```rust
pub struct Mempool {
    // Tüm işlemlerin ana deposu (Hash -> Tx)
    transactions: HashMap<String, Transaction>,

    // Gönderene göre işlemler (Sıralı).
    // Hangi gönderen, hangi nonce sırasına göre işlem atmış?
    // Alice -> [Nonce 5, Nonce 6, Nonce 7]
    by_sender: HashMap<String, BTreeMap<u64, String>>,

    // Ücrete göre işlemler (Sıralı).
    // Madenciler en çok para vereni seçmek ister.
    // Fee 100 -> [TxA, TxB], Fee 50 -> [TxC]
    by_fee: BTreeMap<u64, HashSet<String>>,
}
```

**Neden 3 Farklı Yapı?**
-   `transactions`: İşlemin detayına hızlı erişim (O(1)).
-   `by_sender`: Aynı kullanıcıdan gelen işlemlerin sırasını (Nonce) korumak için. Nonce 5 gelmeden Nonce 6 işlenemez.
-   `by_fee`: Bloğa sığacak en kârlı işlemleri (Greedy Algorithm) seçmek için.

---

## 2. Algoritmalar: Seçim ve Temizlik

### Fonksiyon: `add_transaction` (Kabul Salonu)

```rust
pub fn add_transaction(&mut self, tx: Transaction) -> Result<()> {
    // 1. Zaten var mı?
    if self.transactions.contains_key(&tx.hash) { return Err(...); }

    // 2. Havuz dolu mu? (DDoS Koruması)
    if self.transactions.len() >= self.config.max_size {
        // Havuz doluysa, gelen işlem mevcut en düşük ücretli işlemden 
        // daha mı değerli?
        let min_fee = *self.by_fee.keys().next().unwrap();
        
        if tx.fee > min_fee {
            // Evet daha değerli. Fakir olanı at, zengini al.
            self.evict_lowest_fee();
        } else {
            // Hayır, yeterince para vermemiş. Reddet.
            return Err("Mempool full and fee too low");
        }
    }

    // 3. İndeksleri güncelle.
    self.transactions.insert(tx.hash.clone(), tx.clone());
    self.by_fee.entry(tx.fee).or_default().insert(tx.hash.clone());
    // ...
}
```

### Fonksiyon: `get_sorted_transactions` (Madenci Seçimi)

Madenci blok oluştururken bu fonksiyonu çağırır.

```rust
pub fn get_sorted_transactions(&self, limit: usize) -> Vec<Transaction> {
    let mut selected = Vec::new();
    
    // En yüksek ücretliden (BTreeMap sondan başlar) en düşüğe doğru gez.
    // (Rust BTreeMap keys are sorted ascending, so rev() gives descending)
    for (fee, hashes) in self.by_fee.iter().rev() {
        for hash in hashes {
            if selected.len() >= limit { break; }
            
            // İşlemi al
            let tx = self.transactions.get(hash).unwrap();
            
            // Basitlik için Nonce sırasını şimdilik göz ardı ediyoruz
            // ama gerçekte burada Nonce gap kontrolü de yapılmalı.
            selected.push(tx.clone());
        }
    }
    selected
}
```

---

## 3. RBF (Replace By Fee)

Kullanıcı işleminin takıldığını görürse, aynı nonce ile **daha yüksek ücretli** yeni bir işlem gönderebilir.
`add_transaction` içinde bunu kontrol ederiz:
1.  Gönderenin aynı nonce'lu işlemi var mı?
2.  Varsa, yenisinin ücreti eskisinden %10 fazla mı?
3.  Fazlaysa eskisini sil, yenisini ekle.

Bu mekanizma, "Takılan işlemi kurtarma" (Unsticking Transaction) olarak bilinir.
