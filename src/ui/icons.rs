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
        Gamepad2
    ]
);

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "flags/gb.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/flags/gb.svg"
            )))),
            "flags/ir.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/flags/ir.svg"
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
            ["flags/gb.svg", "flags/ir.svg"]
                .into_iter()
                .filter(|key| key.starts_with(path))
                .map(SharedString::from),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
