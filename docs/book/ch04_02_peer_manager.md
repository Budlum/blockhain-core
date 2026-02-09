# Bölüm 4.2: Eş Yönetimi ve İtibar Sistemi

Bu bölüm, P2P ağındaki "Güven" sorununu matematiksel olarak çözen `PeerManager` ve `PeerScore` yapılarını en ince detayına kadar analiz eder.

Kaynak Dosya: `src/network/peer_manager.rs`

---

## 1. Veri Yapıları: Karne Sistemi

Her eşin (Peer) bir sicili vardır.

### Struct: `PeerScore`

```rust
pub struct PeerScore {
    pub score: i32,                // Puan (-100 ile +100 arası)
    pub banned_until: Option<Instant>, // Ne zamana kadar yasaklı?
    pub invalid_blocks: u32,       // Hatalı blok sayısı
    pub invalid_txs: u32,          // Hatalı işlem sayısı
    pub last_seen: Option<Instant>,// Son görülme
}
```

**Analiz:**
-   `score` (`i32`): Negatif olabileceği için `i32` kullanıldı. Başlangıç puanı 0'dır (Nötr).
-   `banned_until`: `Option` tipindedir. Eğer `None` ise yasaklı değil demektir. Eğer zaman damgası varsa ve o tarih gelecekteyse, o eşten gelen her şey **çöpe atılır** (Drop).

### Sabitler (Constants): Oyunun Kuralları

```rust
const BAN_THRESHOLD: i32 = -100;     // Bu puana düşen banlanır.
const STARTING_SCORE: i32 = 0;       // Yeni gelenin puanı.
const INVALID_BLOCK_PENALTY: i32 = -20; // Büyük suç.
const INVALID_TX_PENALTY: i32 = -5;     // Küçük suç.
const GOOD_BEHAVIOR_REWARD: i32 = 1;    // Ödül (Zor kazanılır).
```

**Neden Bu Değerler?**
-   Bir Node'un banlanması için 5 tane geçersiz blok (`5 * -20 = -100`) yollaması gerekir. Bu, anlık internet kopuklukları veya yazılım hataları (bug) yüzünden dürüst node'ların yanlışlıkla banlanmasını önler (Tolerans Marjı).
-   Ancak puan kazanmak zordur (+1). Güven, damla damla kazanılır, kova kova kaybedilir.

---

## 2. Fonksiyonlar ve Matematik

### Fonksiyon: `report_invalid_block` (Cezalandırma)

Bir eş, kurallara uymayan blok gönderdiğinde çağrılır.

```rust
pub fn report_invalid_block(&mut self, peer_id: &PeerId) {
    // 1. Eşin karnesini getir (Yoksa oluştur).
    let score = self.get_or_create(peer_id);
    
    // 2. Cezayı kes.
    score.score += INVALID_BLOCK_PENALTY; // -20
    score.invalid_blocks += 1;            // İstatistik tut.

    println!("⚠️ Eş {} hatalı blok yolladı. Puanı: {}", peer_id, score.score);

    // 3. Eşik kontrolü: Sınırı aştı mı?
    if score.score <= BAN_THRESHOLD {
        self.ban_peer(peer_id);
    }
}
```

### Fonksiyon: `ban_peer` (Yasaklama)

```rust
fn ban_peer(&mut self, peer_id: &PeerId) {
    let score = self.get_or_create(peer_id);
    
    // 1 saat sonrasını hesapla.
    let ban_duration = Duration::from_secs(3600); 
    score.banned_until = Some(Instant::now() + ban_duration);
    
    println!("🚫 Eş {} BANLANDI! (Süre: 1 Saat)", peer_id);
}
```

---

## 3. Entegrasyon: Nasıl Kullanılır?

Bu sistem `Node::handle_network_event` içinde kullanılır (Bölüm 4.1).

```rust
// Gelen mesajı işlemeden önce:
if self.peer_manager.is_banned(&sender_id) {
    // "Seninle konuşmuyorum."
    return; 
}

// Mesajı işle:
match validate_block(&block) {
    Ok(_) => self.peer_manager.report_good_behavior(&sender_id),
    Err(_) => self.peer_manager.report_invalid_block(&sender_id),
}
```

**Sonuç:**
Bu sistem **otonom** bir bağışıklık sistemidir. İnsan müdahalesi olmadan, ağa saldıranlar otomatik olarak izole edilir.
