use i18n_embed::gettext::{gettext_language_loader, GettextLanguageLoader};
use i18n_embed::DesktopLanguageRequester;
use log::{info, warn};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/mo"]
struct Localizations;

pub fn init() {
    let language_loader: GettextLanguageLoader = gettext_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    info!("Requested languages: {:?}", requested_languages);
    if let Err(e) = i18n_embed::select(&language_loader, &Localizations, &requested_languages) {
        warn!("Failed to load translations: {}", e);
    }
}
