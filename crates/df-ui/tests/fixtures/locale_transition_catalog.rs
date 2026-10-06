use std::collections::BTreeMap;

use df_locale::{
    ArgumentKind, ArgumentValue, CatalogEntry, CatalogLoadError, FormatError, MessagePart, TextKey,
    VersionedCatalog, VersionedFormattedMessage, lookup_chain, settle,
};
use df_types::{Currency, LocaleTag, Money, Usage, UsageUnit};

pub struct CatalogFixture {
    pub locales: [LocaleTag; 2],
    pub keys: [TextKey; 4],
    pub declarations: BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
    pub parts: [[Vec<MessagePart>; 4]; 2],
}

impl CatalogFixture {
    pub fn new() -> Self {
        // These are finite, synthetic caller-selected keys, not a shipped catalog.
        let keys = [
            "transition.location",
            "transition.title",
            "transition.narration",
            "transition.continue",
        ]
        .map(|key| TextKey::parse(key).unwrap());
        let currency = Currency::parse("USD").unwrap();
        let declarations = keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let slots = if index == 2 {
                    BTreeMap::from([
                        ("name".to_owned(), ArgumentKind::Text),
                        ("usage".to_owned(), ArgumentKind::Quantity(UsageUnit::Byte)),
                        ("cost".to_owned(), ArgumentKind::Money(currency)),
                    ])
                } else {
                    BTreeMap::new()
                };
                (key.clone(), slots)
            })
            .collect();
        let parts = [
            [
                "Below Greyhaven",
                "<b>The halls</b>",
                "Survey: ",
                "Enter the halls",
            ],
            [
                "Sous Greyhaven",
                "<b>Les salles</b>",
                "Relevé : ",
                "Entrer dans les salles",
            ],
        ]
        .map(|labels| {
            std::array::from_fn(|index| {
                let mut parts = vec![MessagePart::Literal(labels[index].to_owned())];
                if index == 2 {
                    for (name, kind, separator) in [
                        ("name", ArgumentKind::Text, " / "),
                        ("usage", ArgumentKind::Quantity(UsageUnit::Byte), " / "),
                        ("cost", ArgumentKind::Money(currency), ""),
                    ] {
                        parts.push(MessagePart::Argument {
                            name: name.to_owned(),
                            kind,
                        });
                        parts.push(MessagePart::Literal(separator.to_owned()));
                    }
                }
                parts
            })
        });
        Self {
            locales: [
                LocaleTag::parse("en").unwrap(),
                LocaleTag::parse("fr").unwrap(),
            ],
            keys,
            declarations,
            parts,
        }
    }

    pub fn entries(&self, omitted: Option<(usize, usize)>) -> Vec<CatalogEntry<'_>> {
        let mut entries = Vec::new();
        for (language, locale) in self.locales.iter().enumerate() {
            for (index, key) in self.keys.iter().enumerate() {
                if omitted == Some((language, index)) {
                    continue;
                }
                entries.push(CatalogEntry {
                    locale,
                    key,
                    text: "Synthetic label",
                    parts: &self.parts[language][index],
                });
            }
        }
        entries
    }

    pub fn load(&self, version: u64) -> Result<VersionedCatalog<u64>, CatalogLoadError> {
        VersionedCatalog::load(
            version,
            &self.locales,
            &self.declarations,
            &self.entries(None),
        )
    }

    pub fn messages<'a>(
        &self,
        catalog: &'a VersionedCatalog<u64>,
        requested: &[LocaleTag],
        micros: u128,
    ) -> Result<[VersionedFormattedMessage<'a, u64>; 4], FormatError> {
        let selection = settle(requested, &self.locales, &self.locales[0]).unwrap();
        let chain = lookup_chain(selection.selected(), &self.locales[0]);
        let args = BTreeMap::from([
            (
                "name".to_owned(),
                ArgumentValue::Text("{name}<script>literal</script>".to_owned()),
            ),
            (
                "usage".to_owned(),
                ArgumentValue::Quantity(Usage::new(u128::MAX, UsageUnit::Byte)),
            ),
            (
                "cost".to_owned(),
                ArgumentValue::Money(Money::new(Currency::parse("USD").unwrap(), micros)),
            ),
        ]);
        let empty = BTreeMap::new();
        let [location, title, narration, action] = self.keys.each_ref().map(|key| {
            let arguments = if key == &self.keys[2] { &args } else { &empty };
            catalog.format(key, &chain, arguments)
        });
        Ok([location?, title?, narration?, action?])
    }
}
