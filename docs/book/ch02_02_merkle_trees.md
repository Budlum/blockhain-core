# Bölüm 2.2: Merkle Ağaçları ve Veri Bütünlüğü

Bu bölüm, blok başlığındaki `tx_root` ve `state_root` alanlarının nasıl hesaplandığını, "Hash Ağacı" (Merkle Tree) matematiğini ve hafif istemcilerin (Light Clients) bu yapıyı nasıl kullandığını anlatır.

---

## 1. Kavramsal Temel: Neden Ağaç?

Bir blokta 1 milyon işlem olduğunu düşünün.
-   **Senaryo:** Alice, "Benim işlemim bu blokta var mı?" diye soruyor.
-   **Kötü Çözüm (Düz Liste):** Bloktaki 1 milyon işlemi tek tek indir ve Alice'in işlemini ara. (1 GB veri indirmek gerekir).
-   **Merkle Çözümü:** Sadece Alice'in işleminden kök hash'e giden yolu (Path) indir. (Sadece 1 KB veri gerekir).

### Matematiksel Yapı

Merkle Ağacı, `Hash(Hash(A) + Hash(B))` şeklinde yukarı doğru çıkan bir piramittir. En tepedeki hash'e **Merkle Root** denir.

```text
       ROOT (H7)
      /       \
    H5         H6
   /  \       /  \
 H1    H2   H3    H4
 |     |    |     |
Tx1   Tx2  Tx3   Tx4
```

Eğer `Tx1` değişirse -> `H1` değişir -> `H5` değişir -> `ROOT` değişir.
Kök hash, altındaki milyonlarca yaprağın (Leaf) kriptografik özetidir.

---

## 2. Kod Analizi (`calculate_tx_root`)

Kodumuzda `src/block.rs` içinde `calculate_tx_root` fonksiyonu bu işlemi yapar.

```rust
pub fn calculate_tx_root(&self) -> String {
    // 1. Yaprakları Hazırla: Her işlemin kendi hash'ini al.
    let mut tx_hashes: Vec<String> = self.transactions
        .iter()
        .map(|tx| tx.hash.clone())
        .collect();

    // 2. Boş Blok Kontrolü
    if tx_hashes.is_empty() {
        return "0".repeat(64); // Standart boş kök.
    }

    // 3. Ağacı yukarı doğru ör.
    while tx_hashes.len() > 1 {
        let mut next_level = Vec::new();
        
        // Chunk(2): Listeyi ikişerli gruplara ayır. [A, B], [C, D]...
        for chunk in tx_hashes.chunks(2) {
            let left = &chunk[0];
            
            // Eğer sayı tekse (eşsiz kaldıysa), son elemanı kopyala (A, B, C -> C+C).
            // Bitcoin de böyle yapar.
            let right = if chunk.len() > 1 { &chunk[1] } else { left };

            // 4. İkisini birleştir ve hashle.
            // H(Left + Right)
            let combined = format!("{}{}", left, right);
            let new_hash = hex::encode(hash(combined)); // SHA3-256
            
            next_level.push(new_hash);
        }
        
        // Bir üst kata çık.
        tx_hashes = next_level;
    }

    // 4. Piramidin tepesi (Root).
    tx_hashes[0].clone()
}
```

---

## 3. Light Client (Hafif İstemci) Mantığı

Bir cep telefonu cüzdanı (SPV Wallet) nasıl çalışır?

1.  **Sadece Başlıkları İndir:** Blok başına 1 KB. 10 yıllık zincir bile 50 MB tutar.
2.  **Kök Kontrolü:** Başlıktaki `tx_root` elimizde.
3.  **Kanıt İste:** Full Node'a sor: "Tx1 bu root'un altında mı?"
4.  **Merkle Proof:** Full Node, Tx1'den Root'a giden yolu (`H2`, `H6`) gönderir.
5.  **Yerel Doğrulama:**
    -   Telefon hesaplar: `H1 = Hash(Tx1)`
    -   `H5 = Hash(H1 + H2)` (H2 ağdan geldi)
    -   `Hesaplanan_Root = Hash(H5 + H6)` (H6 ağdan geldi)
    -   Eğer `Hesaplanan_Root == Header.tx_root` ise, işlem %100 buradadır.

Bu sayede 1 TB'lık blok zincirini indirmeden, işlemler kriptografik kesinlikle doğrulanabilir.
