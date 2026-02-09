# Proof of Authority (PoA)

Proof of Authority (Yetki Kanıtı), kimlik tabanlı bir konsensüs mekanizmasıdır. Genellikle özel (private) veya konsorsiyum blok zincirlerinde kullanılır.

Budlum'un PoA uygulaması `src/consensus/poa.rs` dosyasındadır.

## Çalışma Mantığı

PoA'da "madencilik" veya "stake" yoktur. Bunun yerine, önceden belirlenmiş güvenilir düğümler (Otoriteler) vardır.

1.  **Yetkili Listesi:** Blok zincirinin başlangıcında (Genesis) veya oylama ile belirlenen açık anahtarlar listesidir.
2.  **Sıralı Üretim (Round Robin):** Otoriteler sırayla blok üretir.
    -   Örneğin 3 otorite varsa (A, B, C):
    -   Blok 1 -> A
    -   Blok 2 -> B
    -   Blok 3 -> C
    -   Blok 4 -> A ...

## Avantajları

-   **Yüksek Performans:** Karmaşık hesaplamalar (PoW) yoktur. Bloklar çok hızlı üretilir.
-   **Düşük Enerji:** Sadece basit imza doğrulama işlemi yapılır.
-   **Tahmin Edilebilirlik:** Blok üretim süreleri sabittir.

## Dezavantajları

-   **Merkeziyetçilik:** Ağın güvenliği, sınırlı sayıdaki otoriteye emanettir. Bu otoriteler işbirliği yaparsa ağı manipüle edebilirler.
-   **Sansür Riski:** Otoriteler, belirli işlemleri bloklara almayı reddedebilir.

Budlum PoA motoru, blok başlığındaki `producer` alanının yetkili listesinde olup olmadığını ve sırasının gelip gelmediğini kontrol eder.
