# Bölüm 4.3: Ağ Protokolü ve Mesajlaşma

Bu bölüm, makinelerin birbirleriyle konuşurken kullandığı dili (`NetworkMessage`) ve verilerin kablodan geçmeden önce nasıl 0 ve 1'lere dönüştürüldüğünü (Serialization) analiz eder.

Kaynak Dosya: `src/network/protocol.rs` (Varsayımsal)

---

## 1. Veri Yapıları: Ortak Dil

Dünyanın her yerindeki bilgisayarların anlaşabilmesi için ortak bir `Enum` tanımlarız.

### Enum: `NetworkMessage`

Düğümlerin birbirine söyleyebileceği her şey buradadır.

```rust
#[derive(Serialize, Deserialize)] // Serde kütüphanesi
pub enum NetworkMessage {
    // "Elimde yeni bir işlem var, ilgilenir misin?"
    Transaction(Transaction),

    // "Yeni bir blok buldum/onayladım!"
    Block(Block),

    // "Sende Blok 100'den sonrası var mı?" (Senkronizasyon Başlangıcı)
    GetBlocks { start_index: u64 },

    // "Evet var, işte Blok 100-150 arası listesi."
    Blocks(Vec<Block>),
    
    // "Benim versiyonum v1.0, zincir ID'm 1337." (Handshake)
    Hello { version: String, chain_id: u64 },
}
```

**Analiz:**
-   `Transaction` ve `Block` tipleri, Gossipsub (Radyo yayını) üzerinden HERKESE gönderilir.
-   `GetBlocks` ve `Blocks` tipleri, Request/Response (Soru-Cevap) mantığıyla DOĞRUDAN iki kişi arasında konuşulur.

---

## 2. Serileştirme (Serialization)

`Transaction` struct'ı RAM'de duran bir objedir. Kablodan (TCP) gönderilemez. Byte'lara çevrilmelidir.

Budlum projesinde **`bincode`** formatı kullanılmıştır.

### Neden `bincode`? (JSON değil de?)

-   **JSON:** `{"from": "Alice", "amount": 10}` (Okunabilir ama yer kaplar. String işleme yavaştır.)
-   **Bincode:** `05416c6963650a000000...` (Binary. Çok sıkışıktır. CPU dostudur.)

**Performans Farkı:**
Blok zincirinde saniyede binlerce işlem olur. JSON kullanmak, ağı %30-40 yavaşlatır ve CPU'yu yorar. Bincode, Rust struct'larını doğrudan bellekteki haliyle (veya ona çok yakın) diske/ağa yazar.

---

## 3. Limitler ve Güvenlik

Ağdan gelen veri güvenilmezdir. Biri size 10 GB'lık tek bir mesaj yollayıp RAM'inizi patlatabilir (Memory Exhaustion Attack).

**Kod (Gossipsub Config):**
```rust
// libp2p ayarlarında
let gossipsub_config = GossipsubConfigBuilder::default()
    .max_transmit_size(1024 * 1024) // 1 MB Limit
    .build()
    .unwrap();
```

Bu limit sayesinde, 1 MB'tan büyük bloklar veya mesajlar ağda otomatik olarak reddedilir. Bu bir konsensüs kuralıdır. Eğer blok boyutunu artırmak isterseniz, tüm ağın yazılımını güncellemesi (Hard Fork) gerekir.
