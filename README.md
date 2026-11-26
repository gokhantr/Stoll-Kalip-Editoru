# Stoll Kalıp Editörü

[![Sürüm](https://img.shields.io/badge/S%C3%BCr%C3%BCm-v1.0.0-0F766E.svg)](https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/latest)
[![Platform](https://img.shields.io/badge/Platform-Windows%20(x64)-1E3A5F.svg)](https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/latest)
[![Lisans](https://img.shields.io/badge/Lisans-MIT-10B981.svg)](LICENSE)

**Stoll Kalıp Editörü**, Stoll M1plus düz örme makineleri için geliştirilmiş, `.shp` (İlmek) ve `.shv` (mm) kalıp dosyalarını yerel olarak okuyup düzenleyebilen, bağımsız ve açık kaynaklı (MIT) tek parça Windows masaüstü kalıp editörüdür.

Triko desinatörleri ve teknisyenlerinin kalıp hazırlama, ölçü düzenleme, reglan/kavis hesaplama ve dosya dönüştürme iş akışlarını M1plus lisansına bağımlı olmadan hızlandırmak amacıyla **Gökhan Yücel** tarafından geliştirilmiştir.

---

## İndirme (GitHub Releases)

En güncel taşınabilir tek `.exe` sürümünü doğrudan indirip kurulum gerektirmeden çalıştırabilirsiniz:

- **[Stoll Kalıp Editörü — En Son Sürüm (GitHub Releases)](https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/latest)**

> Kurulum gerektirmez. `Stoll_Kalip_Editoru.exe` dosyasını indirip doğrudan çalıştırabilirsiniz; harici DLL bağımlılığı yoktur.

---

## Temel Özellikler

- **Tam `.shp` / `.shv` Uyumluluğu:** UTF-16LE OLE2 Compound File Binary (CFB v3) formatında tam kayıpsız okuma ve M1plus ile birebir uyumlu kayıt.
- **İnteraktif Canlı CAD Kanvası & Dual Tablo:** Sol/sağ çizgiler ve işaretleme noktaları (`İşaretler`), anlık aynalama (`Aynalama`), kapanış kontrolü (`dX=0`) ve canlı ölçü etiketleri.
- **Kavis & Çizgi Editörü:** Bresenham adım dağılımı, J-Eğrisi, S-Eğrisi ve serbest kullanıcı eğrisi (`.crv`) düzenleme.
- **Kalıp Elemanları & Özellikler:** Ana kesim, yaka kesimi, delik, tiktak, açıklık, pens, remayözleme ve kenar düzeltme (8 sekmeli çizgi özellikleri).
- **Format Dönüştürme & Yoğunluk Hesabı:** İlmek $\leftrightarrow$ Basamak dönüşümleri, 1 cm ilmek/sıra yoğunluğu değişiminde otomatik yeniden hesaplama, `.dxf` ve `.csv`/`.tsv` içe/dışa aktarma.
- **Otomatik GitHub Güncelleme Denetimi:** Açılışta arka planda (ve `? -> Hakkında & Güncellemeler` penceresinden manuel olarak) en güncel GitHub sürümünü denetler.

---

## Kaynaktan Derleme

### Gereksinimler
- Rust toolchain (`stable`, `x86_64-pc-windows-msvc` veya `x86_64-pc-windows-gnullvm`)

### Testler ve Derleme

```powershell
# Birim ve entegrasyon testlerini çalıştır
cargo test

# Release ikili dosyasını derle
cargo build --release --bin Stoll_Kalip_Editoru
```

Derlenen çalıştırılabilir dosya `target\release\Stoll_Kalip_Editoru.exe` konumunda oluşturulur.

---

## Lisans

Bu proje [MIT Lisansı](LICENSE) altında açık kaynaklı olarak sunulmuştur.
