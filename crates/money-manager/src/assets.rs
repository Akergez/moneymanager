//! The icons the application draws with.
//!
//! The component library embeds the hundred icons its own widgets use. The
//! rest of the catalogue is a megabyte this application has no use for, so
//! the ones it does draw are named here and embedded by themselves; anything
//! else falls through to the library's bundle.
//!
//! An icon a category can wear has to be in this list as well as in
//! [`crate::book::ICONS`]: one that is only there is a blank tile.

use std::borrow::Cow;

use gpui_kit::assets::{Assets, icon_assets};
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    AppIcons,
    [
        // Navigation and commands.
        List,
        LayoutGrid,
        ChartColumn,
        Plus,
        Pencil,
        RefreshCw,
        Settings,
        Wallet,
        ArrowLeftRight,
        ChevronLeft,
        ChevronRight,
        TriangleAlert,
        Calendar,
        // What a category can wear, in the order of `book::ICONS`.
        Tag,
        Utensils,
        Coffee,
        Pizza,
        Apple,
        Wine,
        Beer,
        ShoppingCart,
        ShoppingBag,
        Shirt,
        Gem,
        Gift,
        Package,
        Car,
        CarTaxiFront,
        Fuel,
        Bus,
        TrainFront,
        Plane,
        Bike,
        House,
        Sofa,
        Wrench,
        Zap,
        Droplet,
        Flame,
        Wifi,
        Smartphone,
        Tv,
        Heart,
        Pill,
        Stethoscope,
        Dumbbell,
        Scissors,
        Baby,
        PawPrint,
        Users,
        Film,
        Music,
        Gamepad2,
        Book,
        Ticket,
        Palette,
        Camera,
        TreePalm,
        Luggage,
        PartyPopper,
        Cake,
        GraduationCap,
        Briefcase,
        Laptop,
        CreditCard,
        Banknote,
        Coins,
        PiggyBank,
        HandCoins,
        Landmark,
        Receipt,
        Percent,
        TrendingUp,
        ShieldCheck,
        Award,
        Store
    ]
);

/// The application's icons, then the component library's.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match AppIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = AppIcons.list(path)?;
        names.extend(Assets.list(path)?);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::ICONS;
    use crate::ui::Lucide;

    #[test]
    fn every_icon_a_category_can_wear_is_embedded() {
        for (key, icon) in ICONS {
            let path = icon.path();
            assert!(
                AppIcons.load(&path).unwrap().is_some(),
                "{key} is offered but {path} is not embedded"
            );
        }
    }

    #[test]
    fn the_icons_the_screens_name_are_embedded() {
        for icon in [
            Lucide::List,
            Lucide::LayoutGrid,
            Lucide::ChartColumn,
            Lucide::ArrowLeftRight,
            Lucide::Wallet,
            Lucide::Pencil,
        ] {
            assert!(AppIcons.load(&icon.path()).unwrap().is_some(), "{icon:?}");
        }
        // And a path that is nobody's is nothing, not an error, here.
        assert!(AppIcons.load("icons/no-such-icon.svg").unwrap().is_none());
    }
}
