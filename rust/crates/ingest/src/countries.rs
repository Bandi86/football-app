//! Közös ország-segédek a forrás-adapterekhez: lokalizált és angol
//! országnév → ISO kód, nemzetközi/kontinens felismerés.

/// Ékezetfüggetlen kisbetűs alak (a zárójelezett hivatalos nevekhez is).
fn fold(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'ő' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'ű' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'š' => 's',
            'ž' => 'z',
            other => other,
        })
        .collect()
}

/// Lokalizált országnév → (ISO, angol név). Ismeretlen → None (fail-open
/// a besorolásnál: a verseny ország nélkül is bekerülhet).
pub fn by_localized_title(title: &str) -> Option<(&'static str, &'static str)> {
    let key = fold(title);
    let pairs: &[(&str, &str, &str)] = &[
        ("anglia", "GB", "England"),
        ("skócia", "GB", "Scotland"),
        ("wales", "GB", "Wales"),
        ("észak-írország", "GB", "Northern Ireland"),
        ("spanyolország", "ES", "Spain"),
        ("olaszország", "IT", "Italy"),
        ("németország", "DE", "Germany"),
        ("franciaország", "FR", "France"),
        ("magyarország", "HU", "Hungary"),
        ("ausztria", "AT", "Austria"),
        ("svájc", "CH", "Switzerland"),
        ("hollandia", "NL", "Netherlands"),
        ("belgium", "BE", "Belgium"),
        ("portugália", "PT", "Portugal"),
        ("törökország", "TR", "Turkey"),
        ("görögország", "GR", "Greece"),
        ("horvátország", "HR", "Croatia"),
        ("szerbia", "RS", "Serbia"),
        ("szlovénia", "SI", "Slovenia"),
        ("bosznia-hercegovina", "BA", "Bosnia and Herzegovina"),
        ("montenegró", "ME", "Montenegro"),
        ("észak-macedónia", "MK", "North Macedonia"),
        ("albánia", "AL", "Albania"),
        ("lengyelország", "PL", "Poland"),
        ("csehország", "CZ", "Czech Republic"),
        ("szlovákia", "SK", "Slovakia"),
        ("románia", "RO", "Romania"),
        ("bulgária", "BG", "Bulgaria"),
        ("oroszország", "RU", "Russia"),
        ("ukrajna", "UA", "Ukraine"),
        ("fehéroroszország", "BY", "Belarus"),
        ("moldova", "MD", "Moldova"),
        ("grúzia", "GE", "Georgia"),
        ("örményország", "AM", "Armenia"),
        ("azerbajdzsán", "AZ", "Azerbaijan"),
        ("kazahsztán", "KZ", "Kazakhstan"),
        ("izrael", "IL", "Israel"),
        ("izland", "IS", "Iceland"),
        ("norvégia", "NO", "Norway"),
        ("svédország", "SE", "Sweden"),
        ("dánia", "DK", "Denmark"),
        ("finnország", "FI", "Finland"),
        ("észtország", "EE", "Estonia"),
        ("lettország", "LV", "Latvia"),
        ("litvánia", "LT", "Lithuania"),
        ("írország", "IE", "Ireland"),
        ("ciprus", "CY", "Cyprus"),
        ("málta", "MT", "Malta"),
        ("luxemburg", "LU", "Luxembourg"),
        ("brazília", "BR", "Brazil"),
        ("argentin", "AR", "Argentina"),
        ("egyesült államok", "US", "United States"),
        ("mexikó", "MX", "Mexico"),
        ("japán", "JP", "Japan"),
        ("kína", "CN", "China"),
        ("mozambik", "MZ", "Mozambique"),
        ("ausztrália", "AU", "Australia"),
        ("egyiptom", "EG", "Egypt"),
        ("marokkó", "MA", "Morocco"),
        ("algéria", "DZ", "Algeria"),
        ("tunézia", "TN", "Tunisia"),
        ("dél-afrika", "ZA", "South Africa"),
        ("antigua & barbuda", "AG", "Antigua and Barbuda"),
        ("antigua and barbuda", "AG", "Antigua and Barbuda"),
        ("argentina", "AR", "Argentina"),
        ("aruba", "AW", "Aruba"),
        ("brazil", "BR", "Brazil"),
        ("cameroon", "CM", "Cameroon"),
        ("chile", "CL", "Chile"),
        ("colombia", "CO", "Colombia"),
        ("dr congo", "CD", "DR Congo"),
        ("england", "GB", "England"),
        ("germany", "DE", "Germany"),
        ("greece", "GR", "Greece"),
        ("hong kong", "HK", "Hong Kong"),
        ("india", "IN", "India"),
        ("iran", "IR", "Iran"),
        ("italy", "IT", "Italy"),
        ("kosovo", "XK", "Kosovo"),
        ("mali", "ML", "Mali"),
        ("mexico", "MX", "Mexico"),
        ("nigeria", "NG", "Nigeria"),
        ("peru", "PE", "Peru"),
        ("portugal", "PT", "Portugal"),
        ("serbia", "RS", "Serbia"),
        ("slovakia", "SK", "Slovakia"),
        ("spain", "ES", "Spain"),
        ("ukraine", "UA", "Ukraine"),
        ("uruguay", "UY", "Uruguay"),
        ("venezuela", "VE", "Venezuela"),
        ("bolívia", "BO", "Bolivia"),
        ("chile", "CL", "Chile"),
        ("uruguay", "UY", "Uruguay"),
        ("honduras", "HN", "Honduras"),
        ("costa rica", "CR", "Costa Rica"),
        ("paraguay", "PY", "Paraguay"),
        ("ecuador", "EC", "Ecuador"),
        ("peru", "PE", "Peru"),
        ("kolumbia", "CO", "Colombia"),
        ("bahrein", "BH", "Bahrain"),
        ("nígéria", "NG", "Nigeria"),
        ("gibraltár", "GI", "Gibraltar"),
        ("szaúd-arábia", "SA", "Saudi Arabia"),
        ("egyesült arab emírségek", "AE", "United Arab Emirates"),
        ("dél-korea", "KR", "South Korea"),
        ("észak-írország", "GB", "Northern Ireland"),
        ("bissau-guinea", "GW", "Guinea-Bissau"),
        ("guinea-bissau", "GW", "Guinea-Bissau"),
        ("elefántcsontpart", "CI", "Ivory Coast"),
        ("dél-szudán", "SS", "South Sudan"),
        ("szomália", "SO", "Somalia"),
        ("jemen", "YE", "Yemen"),
        ("omán", "OM", "Oman"),
        ("kuvait", "KW", "Kuwait"),
        ("jordánia", "JO", "Jordan"),
        ("libanon", "LB", "Lebanon"),
        ("szíria", "SY", "Syria"),
        ("palesztina", "PS", "Palestine"),
        ("irak", "IQ", "Iraq"),
        ("thaiföld", "TH", "Thailand"),
        ("vietnam", "VN", "Vietnam"),
        ("malajzia", "MY", "Malaysia"),
        ("szingapúr", "SG", "Singapore"),
        ("fülöp-szigetek", "PH", "Philippines"),
        ("tádzsikisztán", "TJ", "Tajikistan"),
        ("türkmenisztán", "TM", "Turkmenistan"),
        ("üzbegisztán", "UZ", "Uzbekistan"),
        ("kirgizisztán", "KG", "Kyrgyzstan"),
        ("indonézia", "ID", "Indonesia"),
        ("hongkong", "HK", "Hong Kong"),
        ("iráni", "IR", "Iran"),
        ("iráni iszlám köztársaság", "IR", "Iran"),
        ("afganisztán", "AF", "Afghanistan"),
        ("banglades", "BD", "Bangladesh"),
        ("makaó", "MO", "Macau"),
        ("etiópia", "ET", "Ethiopia"),
        ("egyenlítői-guinea", "GQ", "Equatorial Guinea"),
        ("madagaszkár", "MG", "Madagascar"),
        ("maldív-szigetek", "MV", "Maldives"),
        ("kongói dk", "CD", "DR Congo"),
        ("comore-szigetek", "KM", "Comoros"),
        ("cook-szigetek", "CK", "Cook Islands"),
        ("szent kitts és nevis", "KN", "Saint Kitts and Nevis"),
        ("szent lucia", "LC", "Saint Lucia"),
        ("sint maarten", "SX", "Sint Maarten"),
        ("zöld-foki köztársaság", "CV", "Cape Verde"),
        ("kajmán-szigetek", "KY", "Cayman Islands"),
        ("libéria", "LR", "Liberia"),
        ("ruanda", "RW", "Rwanda"),
        ("burundi", "BI", "Burundi"),
        ("malawi", "MW", "Malawi"),
        ("zambia", "ZM", "Zambia"),
        ("zimbabwe", "ZW", "Zimbabwe"),
        ("botswana", "BW", "Botswana"),
        ("namíbia", "NA", "Namibia"),
        ("lesotho", "LS", "Lesotho"),
        ("szváziföld", "SZ", "Eswatini"),
        ("gambia", "GM", "Gambia"),
        ("mauritánia", "MR", "Mauritania"),
        ("niger", "NE", "Niger"),
        ("csád", "TD", "Chad"),
        ("gabon", "GA", "Gabon"),
        ("kongó", "CG", "Congo"),
        ("dél-afrikai köztársaság", "ZA", "South Africa"),
        ("tanzánia", "TZ", "Tanzania"),
        ("uganda", "UG", "Uganda"),
        ("katar", "QA", "Qatar"),
        ("katari", "QA", "Qatar"),
        ("kanada", "CA", "Canada"),
        ("dominika", "DM", "Dominica"),
        ("dominikai köztársaság", "DO", "Dominican Republic"),
        ("szenegál", "SN", "Senegal"),
        ("laosz", "LA", "Laos"),
        ("feröer-szigetek", "FO", "Faroe Islands"),
        ("pakisztán", "PK", "Pakistan"),
        ("mianmar", "MM", "Myanmar"),
        ("kambodzsa", "KH", "Cambodia"),
        ("ghána", "GH", "Ghana"),
        ("kamerun", "CM", "Cameroon"),
        ("irán", "IR", "Iran"),
        ("vietnám", "VN", "Vietnam"),
        ("zöld-foki szigetek", "CV", "Cape Verde"),
        ("saint kitts és nevis", "KN", "Saint Kitts and Nevis"),
        ("trinidad és tobago", "TT", "Trinidad and Tobago"),
        ("líbia", "LY", "Libya"),
        ("koszovó", "XK", "Kosovo"),
        ("srí lanka", "LK", "Sri Lanka"),
        ("egyesült arab emirátusok", "AE", "United Arab Emirates"),
        ("egye nlítői guinea", "GQ", "Equatorial Guinea"),
        ("egyenlítői guinea", "GQ", "Equatorial Guinea"),
        ("nigéria", "NG", "Nigeria"),
        // Nemzetközi meccseken felbukkanó lokalizált nevek, amelyek eddig
        // ország nélkül maradtak (review-felhalmazódás):
        ("jamaika", "JM", "Jamaica"),
        ("francia guyana", "GF", "French Guiana"),
        ("saint martin", "MF", "Saint Martin"),
        (
            "szent vincent és a grenadine-szigetek",
            "VC",
            "St. Vincent and the Grenadines",
        ),
        (
            "saint vincent és a grenadine-szigetek",
            "VC",
            "St. Vincent and the Grenadines",
        ),
        ("új-kaledónia", "NC", "New Caledonia"),
    ];
    // A leghosszabb illeszkedő név nyer (nem az első a táblában): a
    // részstring-egyezés különben ország-ütközéseket ad — „jordánia” ⊃ „dánia”
    // (JO→DK), „szomália” ⊃ „mali” (SO→ML), „fehéroroszország” ⊃ „oroszország”
    // (BY→RU), „dominikai köztársaság” ⊃ „dominika” (DO→DM). Azonos hosszban
    // a tábla sorrendje dönt (determinisztikus).
    let mut best: Option<(usize, &'static str, &'static str)> = None;
    for (name, cc, cn) in pairs {
        let needle = fold(name);
        if key.contains(&needle) {
            let len = needle.len();
            if best.is_none_or(|(best_len, _, _)| len > best_len) {
                best = Some((len, cc, cn));
            }
        }
    }
    best.map(|(_, cc, cn)| (cc, cn))
}

/// Angol országnevek (nemzetközi meccsek csapatnevei) → (ISO, angol név).
pub fn by_english_name(name: &str) -> Option<(&'static str, &'static str)> {
    let pairs: &[(&str, &str, &str)] = &[
        ("england", "GB", "England"),
        ("scotland", "GB", "Scotland"),
        ("wales", "GB", "Wales"),
        ("northern ireland", "GB", "Northern Ireland"),
        ("spain", "ES", "Spain"),
        ("italy", "IT", "Italy"),
        ("germany", "DE", "Germany"),
        ("france", "FR", "France"),
        ("hungary", "HU", "Hungary"),
        ("austria", "AT", "Austria"),
        ("switzerland", "CH", "Switzerland"),
        ("netherlands", "NL", "Netherlands"),
        ("belgium", "BE", "Belgium"),
        ("portugal", "PT", "Portugal"),
        ("turkey", "TR", "Turkey"),
        ("greece", "GR", "Greece"),
        ("croatia", "HR", "Croatia"),
        ("serbia", "RS", "Serbia"),
        ("slovenia", "SI", "Slovenia"),
        ("bosnia & herzegovina", "BA", "Bosnia and Herzegovina"),
        ("bosnia and herzegovina", "BA", "Bosnia and Herzegovina"),
        ("montenegro", "ME", "Montenegro"),
        ("north macedonia", "MK", "North Macedonia"),
        ("albania", "AL", "Albania"),
        ("poland", "PL", "Poland"),
        ("czech republic", "CZ", "Czech Republic"),
        ("czechia", "CZ", "Czech Republic"),
        ("slovakia", "SK", "Slovakia"),
        ("romania", "RO", "Romania"),
        ("bulgaria", "BG", "Bulgaria"),
        ("russia", "RU", "Russia"),
        ("ukraine", "UA", "Ukraine"),
        ("belarus", "BY", "Belarus"),
        ("moldova", "MD", "Moldova"),
        ("georgia", "GE", "Georgia"),
        ("armenia", "AM", "Armenia"),
        ("azerbaijan", "AZ", "Azerbaijan"),
        ("kazakhstan", "KZ", "Kazakhstan"),
        ("israel", "IL", "Israel"),
        ("iceland", "IS", "Iceland"),
        ("norway", "NO", "Norway"),
        ("sweden", "SE", "Sweden"),
        ("denmark", "DK", "Denmark"),
        ("finland", "FI", "Finland"),
        ("estonia", "EE", "Estonia"),
        ("latvia", "LV", "Latvia"),
        ("lithuania", "LT", "Lithuania"),
        ("ireland", "IE", "Ireland"),
        ("cyprus", "CY", "Cyprus"),
        ("malta", "MT", "Malta"),
        ("luxembourg", "LU", "Luxembourg"),
        ("faroe islands", "FO", "Faroe Islands"),
        ("andorra", "AD", "Andorra"),
        ("gibraltar", "GI", "Gibraltar"),
        ("liechtenstein", "LI", "Liechtenstein"),
        ("san marino", "SM", "San Marino"),
        ("kosovo", "XK", "Kosovo"),
        ("brazil", "BR", "Brazil"),
        ("argentina", "AR", "Argentina"),
        ("uruguay", "UY", "Uruguay"),
        ("colombia", "CO", "Colombia"),
        ("peru", "PE", "Peru"),
        ("chile", "CL", "Chile"),
        ("ecuador", "EC", "Ecuador"),
        ("paraguay", "PY", "Paraguay"),
        ("bolivia", "BO", "Bolivia"),
        ("venezuela", "VE", "Venezuela"),
        ("mexico", "MX", "Mexico"),
        ("united states", "US", "United States"),
        ("usa", "US", "United States"),
        ("canada", "CA", "Canada"),
        ("japan", "JP", "Japan"),
        ("south korea", "KR", "South Korea"),
        ("china", "CN", "China"),
        ("australia", "AU", "Australia"),
        ("egypt", "EG", "Egypt"),
        ("morocco", "MA", "Morocco"),
        ("algeria", "DZ", "Algeria"),
        ("tunisia", "TN", "Tunisia"),
        ("south africa", "ZA", "South Africa"),
        ("nigeria", "NG", "Nigeria"),
        ("ghana", "GH", "Ghana"),
        ("cameroon", "CM", "Cameroon"),
        ("senegal", "SN", "Senegal"),
        ("ivory coast", "CI", "Ivory Coast"),
        ("mali", "ML", "Mali"),
        ("kenya", "KE", "Kenya"),
        ("bermuda", "BM", "Bermuda"),
        ("cuba", "CU", "Cuba"),
        ("grenada", "GD", "Grenada"),
        ("guadeloupe", "GP", "Guadeloupe"),
        ("guyana", "GY", "Guyana"),
        ("tonga", "TO", "Tonga"),
        ("benin", "BJ", "Benin"),
        ("central africa", "CF", "Central African Republic"),
        ("equatorial guinea", "GQ", "Equatorial Guinea"),
        ("eritrea", "ER", "Eritrea"),
        ("guinea", "GN", "Guinea"),
        ("eswatini", "SZ", "Eswatini"),
        ("afghanistan", "AF", "Afghanistan"),
        ("bangladesh", "BD", "Bangladesh"),
        ("brunei", "BN", "Brunei"),
        ("cambodia", "KH", "Cambodia"),
        ("guam", "GU", "Guam"),
        ("laos", "LA", "Laos"),
        ("macau", "MO", "Macau"),
        ("maldives", "MV", "Maldives"),
        ("myanmar", "MM", "Myanmar"),
        ("pakistan", "PK", "Pakistan"),
        ("sri lanka", "LK", "Sri Lanka"),
        ("yemen", "YE", "Yemen"),
        ("libya", "LY", "Libya"),
        ("guinea bissau", "GW", "Guinea-Bissau"),
        ("d.r. congo", "CD", "DR Congo"),
        ("dr congo", "CD", "DR Congo"),
        ("bonaire", "BQ", "Bonaire"),
        ("cayman islands", "KY", "Cayman Islands"),
        ("cook islands", "CK", "Cook Islands"),
        ("martinique", "MQ", "Martinique"),
        ("saint kitts and nevis", "KN", "Saint Kitts and Nevis"),
        ("saint lucia", "LC", "Saint Lucia"),
        ("sint maarten", "SX", "Sint Maarten"),
        ("tahiti", "PF", "Tahiti"),
        ("mozambique", "MZ", "Mozambique"),
        ("ethiopia", "ET", "Ethiopia"),
        ("madagascar", "MG", "Madagascar"),
        ("comoros", "KM", "Comoros"),
        ("curacao", "CW", "Curacao"),
        ("curaçao", "CW", "Curacao"),
        ("guadeloupe", "GP", "Guadeloupe"),
        ("martinique", "MQ", "Martinique"),
        (
            "saint vincent and the grenadines",
            "VC",
            "St. Vincent and the Grenadines",
        ),
        (
            "st. vincent and the grenadines",
            "VC",
            "St. Vincent and the Grenadines",
        ),
        ("bonaire", "BQ", "Bonaire"),
        ("cayman islands", "KY", "Cayman Islands"),
        ("cook islands", "CK", "Cook Islands"),
        ("saint kitts and nevis", "KN", "Saint Kitts and Nevis"),
        ("saint lucia", "LC", "Saint Lucia"),
        ("sint maarten", "SX", "Sint Maarten"),
        ("tahiti", "PF", "Tahiti"),
        ("macau", "MO", "Macau"),
        ("maldives", "MV", "Maldives"),
        ("myanmar", "MM", "Myanmar"),
        ("laos", "LA", "Laos"),
        ("cambodia", "KH", "Cambodia"),
        ("brunei", "BN", "Brunei"),
        ("guam", "GU", "Guam"),
        ("yemen", "YE", "Yemen"),
        ("libya", "LY", "Libya"),
        ("d.r. congo", "CD", "DR Congo"),
        ("guinea bissau", "GW", "Guinea-Bissau"),
        ("bolívia", "BO", "Bolivia"),
        ("kolumbia", "CO", "Colombia"),
        ("bahrein", "BH", "Bahrain"),
        ("nígéria", "NG", "Nigeria"),
        ("gibraltár", "GI", "Gibraltar"),
        ("elefántcsontpart", "CI", "Ivory Coast"),
        ("bissau-guinea", "GW", "Guinea-Bissau"),
        ("gambia", "GM", "Gambia"),
        ("guinea-bissau", "GW", "Guinea-Bissau"),
        ("mauritania", "MR", "Mauritania"),
        ("sierra leone", "SL", "Sierra Leone"),
        ("togo", "TG", "Togo"),
        ("zambia", "ZM", "Zambia"),
        ("angola", "AO", "Angola"),
        ("mozambique", "MZ", "Mozambique"),
        ("ethiopia", "ET", "Ethiopia"),
        ("madagascar", "MG", "Madagascar"),
        ("comoros", "KM", "Comoros"),
        ("curacao", "CW", "Curacao"),
        ("curaçao", "CW", "Curacao"),
        ("guadeloupe", "GP", "Guadeloupe"),
        ("martinique", "MQ", "Martinique"),
        (
            "saint vincent and the grenadines",
            "VC",
            "St. Vincent and the Grenadines",
        ),
        (
            "st. vincent and the grenadines",
            "VC",
            "St. Vincent and the Grenadines",
        ),
        ("bonaire", "BQ", "Bonaire"),
        ("cayman islands", "KY", "Cayman Islands"),
        ("cook islands", "CK", "Cook Islands"),
        ("saint kitts and nevis", "KN", "Saint Kitts and Nevis"),
        ("saint lucia", "LC", "Saint Lucia"),
        ("sint maarten", "SX", "Sint Maarten"),
        ("tahiti", "PF", "Tahiti"),
        ("macau", "MO", "Macau"),
        ("maldives", "MV", "Maldives"),
        ("myanmar", "MM", "Myanmar"),
        ("laos", "LA", "Laos"),
        ("cambodia", "KH", "Cambodia"),
        ("brunei", "BN", "Brunei"),
        ("guam", "GU", "Guam"),
        ("yemen", "YE", "Yemen"),
        ("libya", "LY", "Libya"),
        ("d.r. congo", "CD", "DR Congo"),
        ("guinea bissau", "GW", "Guinea-Bissau"),
        ("madagascar", "MG", "Madagascar"),
        ("comoros", "KM", "Comoros"),
        ("cape verde", "CV", "Cape Verde"),
        ("burkina faso", "BF", "Burkina Faso"),
        ("gabon", "GA", "Gabon"),
        ("congo", "CG", "Congo"),
        ("south sudan", "SS", "South Sudan"),
        ("sudan", "SD", "Sudan"),
        ("liberia", "LR", "Liberia"),
        ("niger", "NE", "Niger"),
        ("chad", "TD", "Chad"),
        ("rwanda", "RW", "Rwanda"),
        ("burundi", "BI", "Burundi"),
        ("ethiopia", "ET", "Ethiopia"),
        ("uganda", "UG", "Uganda"),
        ("tanzania", "TZ", "Tanzania"),
        ("malawi", "MW", "Malawi"),
        ("zimbabwe", "ZW", "Zimbabwe"),
        ("botswana", "BW", "Botswana"),
        ("namibia", "NA", "Namibia"),
        ("lesotho", "LS", "Lesotho"),
        ("seychelles", "SC", "Seychelles"),
        ("somalia", "SO", "Somalia"),
        ("indonesia", "ID", "Indonesia"),
        ("kyrgyzstan", "KG", "Kyrgyzstan"),
        ("uzbekistan", "UZ", "Uzbekistan"),
        ("tajikistan", "TJ", "Tajikistan"),
        ("turkmenistan", "TM", "Turkmenistan"),
        ("thailand", "TH", "Thailand"),
        ("vietnam", "VN", "Vietnam"),
        ("malaysia", "MY", "Malaysia"),
        ("singapore", "SG", "Singapore"),
        ("philippines", "PH", "Philippines"),
        ("india", "IN", "India"),
        ("iraq", "IQ", "Iraq"),
        ("saudi arabia", "SA", "Saudi Arabia"),
        ("qatar", "QA", "Qatar"),
        ("united arab emirates", "AE", "United Arab Emirates"),
        ("oman", "OM", "Oman"),
        ("bahrain", "BH", "Bahrain"),
        ("kuwait", "KW", "Kuwait"),
        ("jordan", "JO", "Jordan"),
        ("lebanon", "LB", "Lebanon"),
        ("syria", "SY", "Syria"),
        ("palestine", "PS", "Palestine"),
        ("puerto rico", "PR", "Puerto Rico"),
        ("haiti", "HT", "Haiti"),
        ("trinidad & tobago", "TT", "Trinidad and Tobago"),
        ("trinidad and tobago", "TT", "Trinidad and Tobago"),
        ("suriname", "SR", "Suriname"),
        ("dominica", "DM", "Dominica"),
        ("dominican republic", "DO", "Dominican Republic"),
        (
            "st. vincent & grenadines",
            "VC",
            "Saint Vincent and the Grenadines",
        ),
        ("st. lucia", "LC", "Saint Lucia"),
        ("st. kitts & nevis", "KN", "Saint Kitts and Nevis"),
        ("montserrat", "MS", "Montserrat"),
        ("bahamas", "BS", "Bahamas"),
        ("barbados", "BB", "Barbados"),
        ("belize", "BZ", "Belize"),
        ("costa rica", "CR", "Costa Rica"),
        ("el salvador", "SV", "El Salvador"),
        ("guatemala", "GT", "Guatemala"),
        ("honduras", "HN", "Honduras"),
        ("jamaica", "JM", "Jamaica"),
        ("nicaragua", "NI", "Nicaragua"),
        ("panama", "PA", "Panama"),
        ("curacao", "CW", "Curacao"),
        ("aruba", "AW", "Aruba"),
        ("french guiana", "GF", "French Guiana"),
        ("saint martin", "MF", "Saint Martin"),
        ("new caledonia", "NC", "New Caledonia"),
    ];
    let key = fold(name);
    pairs
        .iter()
        .find(|(label, _, _)| key == fold(label))
        .map(|(_, cc, cn)| (*cc, *cn))
}

/// Nemzetközi verseny-e (a csapatnév ekkor ország-név).
pub fn is_international_text(text: &str) -> bool {
    [
        "international",
        "nations league",
        "cup of nations",
        "qualification",
        "world cup",
        "friendly",
        "euro u",
        "copa america",
        "afcon",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// Kontinens/világ zászló → nemzetközi verseny (a csapatnév ország).
///
/// egzakt (foldolt) egyezés, NEM részstring: a „Dél-Afrika” és „Közép-Afrika”
/// zászló-cím tartalmazza az „afrika” szót, de ország-zászlók, nem kontinensé.
pub fn is_continent_flag(title: &str) -> bool {
    let key = fold(title);
    [
        "europe",
        "európa",
        "africa",
        "afrika",
        "world",
        "világ",
        "north & central america",
        "north america",
        "é. és k.-amerika",
        "észak- és közép-amerika",
        "dél-amerika",
        "karib",
        "south america",
        "asia",
        "ázsia",
        "oceania",
        "óceánia",
        "international",
    ]
    .iter()
    .any(|needle| key == fold(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ország-ütközési regressziók: a rövidebb név részstringje a hosszabbnak,
    /// és a tábla sorrendje korábban eldöntötte, melyik nyer (rosszul).
    #[test]
    fn longest_match_wins_over_substring_collisions() {
        // jordánia ⊃ dánia → JO (volt: DK, élő hiba 2026-11-19)
        assert_eq!(by_localized_title("Jordánia"), Some(("JO", "Jordan")));
        assert_eq!(by_localized_title("Dánia"), Some(("DK", "Denmark")));
        // szomália ⊃ mali → SO (volt: ML)
        assert_eq!(by_localized_title("Szomália"), Some(("SO", "Somalia")));
        assert_eq!(by_localized_title("Mali"), Some(("ML", "Mali")));
        // fehéroroszország ⊃ oroszország → BY (volt: RU)
        assert_eq!(
            by_localized_title("Fehéroroszország"),
            Some(("BY", "Belarus"))
        );
        assert_eq!(by_localized_title("Oroszország"), Some(("RU", "Russia")));
        // dominikai köztársaság ⊃ dominika → DO (volt: DM)
        assert_eq!(
            by_localized_title("Dominikai Köztársaság"),
            Some(("DO", "Dominican Republic"))
        );
        assert_eq!(by_localized_title("Dominika"), Some(("DM", "Dominica")));
        // románia ⊃ omán → RO (a tábla sorrendje eddig védett, törékeny volt)
        assert_eq!(by_localized_title("Románia"), Some(("RO", "Romania")));
        // nigéria ⊃ niger → NG
        assert_eq!(by_localized_title("Nigéria"), Some(("NG", "Nigeria")));
        assert_eq!(by_localized_title("Niger"), Some(("NE", "Niger")));
        // kongói dk ⊃ kongó → CD
        assert_eq!(by_localized_title("Kongói DK"), Some(("CD", "DR Congo")));
        assert_eq!(by_localized_title("Kongó"), Some(("CG", "Congo")));
        // észak-írország ⊃ írország → GB
        assert_eq!(
            by_localized_title("Észak-Írország"),
            Some(("GB", "Northern Ireland"))
        );
        assert_eq!(by_localized_title("Írország"), Some(("IE", "Ireland")));
    }

    #[test]
    fn decorated_titles_still_resolve() {
        assert_eq!(
            by_localized_title("Anglia - Premier League"),
            Some(("GB", "England"))
        );
        assert_eq!(
            by_localized_title("  Spanyolország  "),
            Some(("ES", "Spain"))
        );
        assert_eq!(by_localized_title("Ismeretlen Terület"), None);
        assert_eq!(by_localized_title(""), None);
    }

    /// A nemzetközi meccseken ország nélkül maradt lokalizált nevek pótlása.
    #[test]
    fn previously_missing_localized_names() {
        assert_eq!(by_localized_title("Jamaika"), Some(("JM", "Jamaica")));
        assert_eq!(
            by_localized_title("Francia Guyana"),
            Some(("GF", "French Guiana"))
        );
        assert_eq!(
            by_localized_title("Saint Martin"),
            Some(("MF", "Saint Martin"))
        );
        assert_eq!(
            by_localized_title("Saint Vincent és a Grenadine-szigetek"),
            Some(("VC", "St. Vincent and the Grenadines"))
        );
        // A holland oldal (Sint Maarten) külön entitás marad.
        assert_eq!(
            by_localized_title("Sint Maarten"),
            Some(("SX", "Sint Maarten"))
        );
        assert_eq!(
            by_english_name("French Guiana"),
            Some(("GF", "French Guiana"))
        );
        assert_eq!(
            by_english_name("Saint Martin"),
            Some(("MF", "Saint Martin"))
        );
        assert_eq!(by_english_name("Jamaica"), Some(("JM", "Jamaica")));
        assert_eq!(
            by_localized_title("Új-Kaledónia"),
            Some(("NC", "New Caledonia"))
        );
        assert_eq!(
            by_english_name("New Caledonia"),
            Some(("NC", "New Caledonia"))
        );
    }

    /// A kontinens-zászló egzakt egyezés: az „afrika” részstring miatt a
    /// Dél-Afrika / Közép-Afrika ország-zászló nem lehet nemzetközi jelző.
    #[test]
    fn continent_flag_is_exact_not_substring() {
        assert!(is_continent_flag("Afrika"));
        assert!(is_continent_flag("Európa"));
        assert!(is_continent_flag("Ázsia"));
        assert!(is_continent_flag("Dél-Amerika"));
        assert!(is_continent_flag("Világ"));
        assert!(is_continent_flag("Europe"));
        assert!(!is_continent_flag("Dél-Afrika"));
        assert!(!is_continent_flag("Közép-Afrika"));
        assert!(!is_continent_flag("Dánia"));
        assert!(!is_continent_flag("Jordánia"));
        assert!(!is_continent_flag(""));
    }
}
