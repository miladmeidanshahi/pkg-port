//! Asset source: gpui-kit's icon set plus the extra icons this app uses and
//! the language flag icons.

use gpui_kit::assets;
use gpui_kit::*;
use std::borrow::Cow;

pub struct AppAssets;

assets::icon_assets!(
    ExtraIcons,
    [
        Download,
        Package,
        Monitor,
        PackageCheck,
        Shield,
        Upload,
        Gamepad2,
        MessageCircleQuestionMark
    ]
);

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icon.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../../assets/icon.svg")))),
            "flags/gb.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/flags/gb.svg"
            )))),
            "flags/ir.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/flags/ir.svg"
            )))),
            "guides/ps4/6.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-6.jpg"
            )))),
            "guides/ps4/7.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-7.jpg"
            )))),
            "guides/ps4/8.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-8.jpg"
            )))),
            "guides/ps4/9.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-9.jpg"
            )))),
            "guides/ps4/10.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-10.jpg"
            )))),
            "guides/ps4/11.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-11.jpg"
            )))),
            "guides/ps4/12.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-12.jpg"
            )))),
            "guides/ps4/13.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-13.jpg"
            )))),
            "guides/ps4/14.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-14.jpg"
            )))),
            "guides/ps4/15.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-15.jpg"
            )))),
            "guides/ps4/16.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-16.jpg"
            )))),
            "guides/ps4/17.jpg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps4/pkgport-ps4-17.jpg"
            )))),
            "guides/ps5/1.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-1.webp"
            )))),
            "guides/ps5/2.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-2.webp"
            )))),
            "guides/ps5/3.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-3.webp"
            )))),
            "guides/ps5/4.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-4.webp"
            )))),
            "guides/ps5/5.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-5.webp"
            )))),
            "guides/ps5/6.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-6.webp"
            )))),
            "guides/ps5/7.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-7.webp"
            )))),
            "guides/ps5/8.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-8.webp"
            )))),
            "guides/ps5/9.webp" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/guides/ps5/pkgport-ps5-9.webp"
            )))),
            _ => match ExtraIcons.load(path)? {
                Some(icon) => Ok(Some(icon)),
                None => assets::Assets.load(path),
            },
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.extend(
            [
                "icon.svg",
                "flags/gb.svg",
                "flags/ir.svg",
                "guides/ps4/6.jpg",
                "guides/ps4/7.jpg",
                "guides/ps4/8.jpg",
                "guides/ps4/9.jpg",
                "guides/ps4/10.jpg",
                "guides/ps4/11.jpg",
                "guides/ps4/12.jpg",
                "guides/ps4/13.jpg",
                "guides/ps4/14.jpg",
                "guides/ps4/15.jpg",
                "guides/ps4/16.jpg",
                "guides/ps4/17.jpg",
                "guides/ps5/1.webp",
                "guides/ps5/2.webp",
                "guides/ps5/3.webp",
                "guides/ps5/4.webp",
                "guides/ps5/5.webp",
                "guides/ps5/6.webp",
                "guides/ps5/7.webp",
                "guides/ps5/8.webp",
                "guides/ps5/9.webp",
            ]
            .into_iter()
            .filter(|key| key.starts_with(path))
            .map(SharedString::from),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
