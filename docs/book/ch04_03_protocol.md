# Bölüm 4.3: Ağ Protokolü ve Mesajlaşma

Bu bölüm, makinelerin birbirleriyle konuşurken kullandığı dili (`NetworkMessage`) ve verilerin kablodan geçmeden önce nasıl 0 ve 1'lere dönüştürüldüğünü (Serialization) analiz eder.

Kaynak Dosya: `src/network/protocol.rs` (Varsayımsal)

---

## 1. Veri Yapıları: Ortak Dil

Dünyanın her yerindeki bilgisayarların anlaşabilmesi için ortak bir `Enum` tanımlarız.
# Protokol Mesajları ve Serileştirme (Serialization)

`budlum-core` içindeki node'lar, eşler düzeyindeki (p2p) ağı yönetmek ve veri paylaşmak için özel `NetworkMessage` protokolünü kullanırlar.

## `NetworkMessage` Neler İçerir?

Ağdaki tüm iletişim bir enum (numaralandırılmış yapı) üzerinden geçer. En önemli türleri şunlardır:

1.  **El Sıkışma (Handshake / HandshakeAck)**: Ağa yeni katılanlar bağlanırken versiyon ve `chain_id` bilgilerini doğrularlar. Hatalı `chain_id` anında engellenir (Ban).
2.  **Block**: Yeni çıkarılan bir bloğun tüm peer'lara (eşlere) yayılması.
3.  **Transaction**: Kullanıcılar tarafından oluşturulan ve doğrulanmış (imza, bakiye vb.) yeni bir işlemin mempool'lara (işlem havuzu) yayılması.
4.  **NewTip**: Bir node, blok zincirinde yeni bir yüksekliğe ulaştığında, diğer düğümleri haberdar etmek için o bloğun hash ve yüksekliğini (height) gönderir.
5.  **GetBlocksByHeight / BlocksByHeight (Snap-Sync)**: Zincirin gerisinde kalan bir düğümün, `NewTip` duyduğunda kendi yüksekliğinden itibaren yeni blokları 256'şar parçalar (chunk) halinde topluca istemesini (ve almasını) sağlayan hızlı senkronizasyon mesajlarıdır.
6.  **GetStateSnapshot / StateSnapshotResponse**: Node'ların hızlı doğrulama için belli yüksekliklerdeki `state_root` özetini öğrenme taleplerini yönetir.

*Tam Liste kaynak kodu üzerinden incelenebilir: `src/network/protocol.rs`*

## GossipSub ile Yayın Yapma (Publish)

Budlum Core iletişimi **GossipSub** üzerinden yürütür. Doğrudan tek bir node'a mesaj göndermek yerine (TCP Direct Stream harici), belli konu başlıklarına (örneğin "blocks" veya "transactions") mesaj yayımlanır. Kütüphane optimalliği sayesinde bu mesaj saniyeler içinde ağdaki tüm düğümlere dedikodu ("gossip") yöntemiyle ulaşır.

## Serileştirme (Serde)

Mesajlar ağ üzerine bayt olarak çıkmadan önce serileştirilir.
`budlum-core`, Rust ekosistemindeki en popüler formatlama kütüphanesi olan `serde` ve `serde_json` kullanır. İlerleyen güncellemelerde daha küçük bant genişliği kullanmak amacıyla `bincode` veya `protobuf`'a geçiş hedeflenmektedir. O anki (Handshake ve Sync paketi harici) standart payload boyutu metin tabanlı JSON ile yönetilmektedir.
ce", "amount": 10}` (Okunabilir ama yer kaplar. String işleme yavaştır.)
-   **Bincode:** `05416c6963650a000000...` (Binary. Çok sıkışıktır. CPU dostudur.)

**Performans Farkı:**
Blok zincirinde saniyede binlerce işlem olur. JSON kullanmak, ağı %30-40 yavaşlatır ve CPU'yu yorar. Bincode, Rust struct'larını doğrudan bellekteki haliyle (ve ona çok yakın) diske/ağa yazar.

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
