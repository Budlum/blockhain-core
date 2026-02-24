# Bölüm 4.2: Eş Yönetimi, İtibar Sistemi ve Ağ Koruması

Bu bölüm, P2P ağındaki "Güven" sorununu matematiksel olarak çözen `PeerManager` ve `PeerScore` yapılarını en ince detayına kadar analiz eder. Ağa yeni eklenen **Token-Bucket Rate Limiting** mekanizması ile düğümler DDOS saldırılarından kendini korur.

Kaynak Dosya: `src/network/peer_manager.rs`

---

## 1. Veri Yapıları: Karne ve Hız Limiti Sistemi

Her eşin (Peer) bir sicili ve mesajlaşma kapasitesi (Bucket) vardır.

### Struct: `PeerScore`

```rust
pub struct PeerScore {
    pub score: i32,                // Puan (-100 ile +100 arası)
    pub banned_until: Option<Instant>, // Ne zamana kadar yasaklı?
    pub invalid_blocks: u32,       // Hatalı blok sayısı
    pub invalid_txs: u32,          // Hatalı işlem sayısı
    pub rate_tokens: f64,          // Kalan mesaj hakkı (Token Bucket)
    pub rate_last_refill: Instant, // Jetonların (Token) son yenilenme zamanı
    pub last_seen: Option<Instant>,// Son görülme
}
```

**Analiz:**
-   `score` (`i32`): Negatif olabileceği için `i32` kullanıldı. Başlangıç puanı 0'dır (Nötr).
-   `banned_until`: `Option` tipindedir. Eğer `None` ise yasaklı değil demektir. Eğer zaman damgası varsa ve o tarih gelecekteyse, o eşten gelen her şey **çöpe atılır** (Drop).
-   `rate_tokens` & `rate_last_refill`: "Token-Bucket" algoritmasının ana değişkenleri. Her bir peer'ın belirli bir mesaj kotası (örn. saniyede 5) vardır.

### Sabitler (Constants): Oyunun Kuralları

```rust
const BAN_THRESHOLD: i32 = -100;     // Bu puana düşen banlanır.
const STARTING_SCORE: i32 = 0;       // Yeni gelenin puanı.
const INVALID_BLOCK_PENALTY: i32 = -20; // Büyük suç.
const INVALID_TX_PENALTY: i32 = -5;     // Küçük suç.
const GOOD_BEHAVIOR_REWARD: i32 = 1;    // Ödül (Zor kazanılır).

// Rate Limiting Sabitleri
const RATE_LIMIT_CAPACITY: f64 = 20.0;    // Maksimum birikebilecek jeton (Burst)
const RATE_LIMIT_REFILL_RATE: f64 = 5.0;  // Saniyede yenilenen jeton sayısı
```

**Neden Bu Değerler?**
-   Bir Node'un banlanması için 5 tane geçersiz blok (`5 * -20 = -100`) yollaması gerekir. Bu, anlık internet kopuklukları veya yazılım hataları (bug) yüzünden dürüst node'ların yanlışlıkla banlanmasını önler (Tolerans Marjı).
-   Ancak puan kazanmak zordur (+1). Güven, damla damla kazanılır, kova kova kaybedilir.
-   Spam/Flood saldırısına karşı bir saniyede en fazla 5 mesaj işlenir. Burst kapasitesi (anlık yoğunluk) 20 mesajdır. Bu limiti aşan mesajlar yoksayılır ve hatta gönderici puan kaybeder.

---

## 2. Fonksiyonlar ve Matematik

### Fonksiyon: `check_rate_limit` (Spam Koruması)

Bir eşin mesaj atma hakkı (jetonu) olup olmadığını hesaplar. Jeton (Token) eksikse mesaj düşürülür.

```rust
pub fn check_rate_limit(&mut self, peer_id: &PeerId) -> bool {
    let score = self.get_or_create(peer_id);
    let now = Instant::now();
    let elapsed = now.duration_since(score.rate_last_refill).as_secs_f64();
    
    // Geçen süreye göre jetonları yenile (refill)
    score.rate_tokens = (score.rate_tokens + elapsed * RATE_LIMIT_REFILL_RATE)
        .min(RATE_LIMIT_CAPACITY);
    score.rate_last_refill = now;

    if score.rate_tokens >= 1.0 {
        score.rate_tokens -= 1.0;
        true // İzin verildi
    } else {
        // İzin reddedildi. Çok spam yapanı cezalandır.
        self.report_oversized_message(peer_id);
        false
    }
}
```


### Fonksiyon: `report_invalid_block` (Cezalandırma)

Bir eş, kurallara uymayan blok gönderdiğinde çağrılır.

```rust
pub fn report_invalid_block(&mut self, peer_id: &PeerId) {
    // 1. Eşin karnesini getir (Yoksa oluştur).
    let score = self.get_or_create(peer_id);
    
    // 2. Cezayı kes.
    score.score += INVALID_BLOCK_PENALTY; // -20
    score.invalid_blocks += 1;            // İstatistik tut.

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
}
```

---

## 3. Entegrasyon: Nasıl Kullanılır?

Bu sistem `Node::handle_network_event` içinde kullanılır (Bölüm 4.1).

```rust
// Gelen mesajı işlemeden önce:
if self.peer_manager.lock().unwrap().is_banned(&sender_id) {
    return; // "Seninle konuşmuyorum."
}

if !self.peer_manager.lock().unwrap().check_rate_limit(&sender_id) {
    return; // "Çok hızlı konuşuyorsun, yavaşla."
}

// Mesajı işle:
match chain.validate_and_add_block(block) {
    Ok(_) => self.peer_manager.lock().unwrap().report_good_behavior(&sender_id),
    Err(_) => self.peer_manager.lock().unwrap().report_invalid_block(&sender_id),
}
```

**Sonuç:**
Bu sistem **otonom** bir bağışıklık sistemidir. İnsan müdahalesi olmadan, ağa saldıranlar ve flood yapan botlar otomatik olarak tespit edilir, cezalandırılır ve engellenir.
