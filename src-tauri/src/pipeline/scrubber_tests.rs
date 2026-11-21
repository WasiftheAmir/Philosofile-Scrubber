use super::*;

fn scrub_plain(s: &str) -> ScrubReport {
    scrub_text_with(s, &ScrubOptions::without_identity())
}

const PASSPORT_OCR: &str = "**: ::: ::: •:••: PERSONAL DATA AND EMERGENCY CONTACT Name: TARIQ KHAN Father's Name: Mother's Name: Spouse's Name: Permanent Adid/ess: MD AHMED KHAN FATEMA BEGUM NA VILLAGE KAZIPUR, POST SHANTIPUR, RANGPUR SADAR, RANGPUR Emergency Contact Name: MD AHMED KHAN Relationship: FATHER Address: APT.B2, H# 123, GREEN ROAD, DHANMONDI, DHAKA Telephone No: 01711000000 PASSPORT No. AB 1234567 KAMAL RAHMAN Deputy \"ssistant Piector Department oi Immugrater & Passporte Govt. of the People's Republic of Banglaresh Dhaka PASSPORT 592(97|021 7187) People's Republie of Bangladesh i/ Type GP*t CPTG / Country Code 9/A / Passport No. P BGD AB1234567 4K°910 SINY Sumame KHAN 4И 8 H|4 / Given Name TARIQ TORTel/ Nationality BANGLADESHI 3iI 019*/ Date of Birth 15 MAY 1995 fe1107/ Personal No. 19951234567890123 frol 2/709 / Previous Passport No. XY9876543 Sex 9*121 / Place of Birth M DHAKA erica Sif/ Date of Issue 02 JAN 2020 CHRICHTGIC IR4 / Date of Expiry 01 JAN 2025 DIP / DHAKA *194 / Holder's Signature Tariq P<BGDKHAN<<TARIQ<<<<<<<<<<<<<<<<<<<<<<<<<< AB12345678BGD9505151M2501017<< <<06";

#[test]
fn test_1_passport_has_no_leaks() {
    let out = scrub_plain(PASSPORT_OCR).text;
    for leak in [
        "TARIQ",
        "KHAN",
        "AHMED",
        "BEGUM",
        "01711000000",
        "AB 1234567",
        "AB1234567",
        "XY9876543",
        "15 MAY 1995",
        "19951234567890123",
        "DHAKA",
        "P<BGDKHAN",
    ] {
        assert!(!out.contains(leak), "leaked {:?} in: {}", leak, out);
    }
}

#[test]
fn test_address_line_trailing_place_not_leaked() {
    let input = "Address: Unit.Z9, Sector 4, River Road, Oakhaven Telephone No: 555-0199";
    let out = scrub_plain(input).text;
    assert!(
        !out.contains("Oakhaven"),
        "Expected invented place name at end of address to be redacted, got: {}",
        out
    );
    assert!(out.contains("[ADDRESS]"), "Expected address tag, got: {}", out);
}

#[test]
fn test_2_ubc_card_with_names() {
    let opts = ScrubOptions::new(&["alex".to_string(), "mercer".to_string()]);
    let out = scrub_text_with(
        "UBCcard Undergraduate Alex Mercer 99887766 UBC THE UNIVERSITY OF BRITISH COLUMBIA",
        &opts,
    )
    .text;
    for leak in ["Alex", "Mercer", "99887766"] {
        assert!(!out.contains(leak), "leaked {:?} in: {}", leak, out);
    }
}

#[test]
fn test_3_ssn_card_luhn() {
    assert!(scrub_plain("My number is 123-45-6789 ok")
        .text
        .contains("[SSN]"));
    assert!(scrub_plain("card 4111 1111 1111 1111 ok")
        .text
        .contains("[CARD]"));
    // bad Luhn: never a [CARD]; the digits must still not survive (phone/catch-all take them)
    let bad = scrub_plain("card 4111 1111 1111 1112 ok").text;
    assert!(!bad.contains("[CARD]"), "{}", bad);
    assert!(!bad.contains("4111"), "{}", bad);
}

#[test]
fn test_4_email_and_phone() {
    let input = "Contact user.name+tag@philosofile.com or call 555-123-4567 or (800) 555-0199 or +1-888-555-1212 regarding SSN.";
    assert_eq!(
        scrub_plain(input).text,
        "Contact [EMAIL] or call [PHONE] or [PHONE] or [PHONE] regarding SSN."
    );
}

#[test]
fn test_5_clean_sentence_unchanged() {
    let s = "Quarterly budget review meeting notes for the design team";
    assert_eq!(scrub_plain(s).text, s);
}

#[test]
fn test_6_over_redaction_regression() {
    let s = "The quarterly budget review covers all department expenditures for the fiscal year. Please submit your expense reports by the end of the month. Questions about the schedule can be raised at the next planning meeting.";
    assert_eq!(scrub_plain(s).text, s);
}

#[test]
fn test_7_context_required_score_floor() {
    // 17 digits: outside the phone rule's 9-15 digit range, matches BD NID (context_required)
    // but with no context keyword it must NOT earn a National ID label.
    let out = scrub_plain("The value is 12345678901234567.").text;
    assert!(out.contains("[ID_NUMBER]"), "{}", out);
    assert!(!out.contains("[National ID Number]"), "{}", out);
    assert!(!out.contains("[Bangladeshi National ID Number]"), "{}", out);
}

#[test]
fn test_registry_context_gives_label() {
    let out = scrub_plain("Applicant NID 1234567890 was received in Bangladesh").text;
    assert!(out.contains("[Bangladeshi National ID Number]"), "{}", out);
}

#[test]
fn test_registry_csv_is_valid() {
    match check_registry() {
        Ok(n) => assert!(n > 0),
        Err(e) => panic!("id_formats.csv invalid:\n{}", e.join("\n")),
    }
}

#[test]
fn test_replace_no_lookbehind_keeps_boundary() {
    assert_eq!(
        replace_no_lookbehind("ID:123-45-6789.", &SSN_RE, "[SSN]"),
        "ID:[SSN]."
    );
}

#[test]
fn test_normalize_keeps_newlines() {
    assert_eq!(normalize("a  \t b\r\nc"), "a b\nc");
}

#[test]
fn test_mrz_variants() {
    let (t, f) = scrub_mrz("x P<BGDTESTER<<SAMPLE<<<<<<<<<<<<<<<<<<<<<<< y");
    assert!(f && t.contains("[MRZ]"));
    // OCR inserted a space; needs >= 4 '<'
    let (t, f) = scrub_mrz("AB12345678BGD9505151M2501017<< <<06");
    assert!(f && t == "[MRZ]", "{}", t);
    // ordinary uppercase text must not match
    let (t, f) = scrub_mrz("THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG AGAIN");
    assert!(!f && t.contains("QUICK"));
}

#[test]
fn test_catch_all_partial_placeholder() {
    // A token that is not ENTIRELY a placeholder is still evaluated.
    let (t, _) = scrub_catch_all("see [DATE]-123456 now", false);
    assert!(t.contains("[ID_NUMBER]") && !t.contains("123456"), "{}", t);
    let (t, _) = scrub_catch_all("see [DATE] now", false);
    assert_eq!(t, "see [DATE] now");
}

#[test]
fn test_signature_on_next_line() {
    let out =
        scrub_plain("Thanks for the update.\n\nBest regards,\nJohn Smith\nSent from my phone")
            .text;
    assert!(!out.contains("John") && !out.contains("Smith"), "{}", out);
}

#[test]
fn test_heading_not_redacted() {
    let s = "Project Overview: the plan covers three phases.";
    assert_eq!(scrub_plain(s).text, s);
}

#[test]
fn test_span_dedup_single_tag() {
    let out = scrub_plain("nid 1234567890").text;
    assert_eq!(out.matches('[').count(), 1, "{}", out);
}

#[test]
fn test_aggressive_mode_floor() {
    // high-sensitivity doc: a bare 10-digit number with no context keyword is redacted
    let out = scrub_plain("Passport copy attached. Reference value 1234567890 end.").text;
    assert!(!out.contains("1234567890"), "{}", out);
}

#[test]
fn test_checksums() {
    assert!(luhn("4111111111111111") && !luhn("4111111111111112"));
    assert!(verhoeff("2363") && !verhoeff("2364"));
    assert!(mod97("GB82WEST12345698765432") && !mod97("GB82WEST12345698765433"));
    assert!(aba("011401533") && !aba("011401534"));
    assert!(mod11_nhs("9434765919") && !mod11_nhs("9434765918"));
}

#[test]
fn test_report_counts_use_string_keys() {
    let r = scrub_plain("mail a@b.com and c@d.org");
    assert_eq!(r.counts.get("EMAIL"), Some(&2));
}

mod baseline_issues {
    use super::super::*;

    fn scrub_plain(s: &str) -> ScrubReport {
        scrub_text_with(s, &ScrubOptions::without_identity())
    }

    /// Tests that a lowercase invented name following an ID placeholder inside a label block
    /// is still redacted and not dropped when label scanning stops at `[`.
    #[test]
    fn test_name_after_id_placeholder_in_label() {
        let input = "Passport No: A12345678 frodo baggins";
        let out = scrub_plain(input).text;
        assert!(
            !out.contains("frodo") && !out.contains("baggins"),
            "Expected invented name to be redacted, but got: {}",
            out
        );
    }

    /// Tests that a Bengali name following an ID placeholder inside a label block
    /// is redacted even when label scanning stops at `[`.
    #[test]
    fn test_bengali_name_after_id_placeholder_in_label() {
        let input = "Passport No: A12345678 অনিক মজুমদার";
        let out = scrub_plain(input).text;
        assert!(
            !out.contains("অনিক") && !out.contains("মজুমদার"),
            "Expected invented Bengali name to be redacted, but got: {}",
            out
        );
    }

    /// Tests that short context keywords (e.g. "tin" inside "continue") do not match
    /// as substrings to elevate unrelated numbers to specific identifier labels.
    #[test]
    fn test_short_keyword_false_positives() {
        let input = "Please continue to process 123456789012 for the business.";
        let out = scrub_plain(input).text;
        assert!(
            !out.contains("[Tax ID Number]") && !out.contains("[Social Security Number]"),
            "Expected no specific identifier label assigned from substring match, but got: {}",
            out
        );
    }

    /// Tests that Bengali Unicode digits (০-৯) are recognized and redacted consistently
    /// with ASCII digits by the registry and catch-all pipeline.
    #[test]
    fn test_bengali_digit_counting() {
        let input = "জাতীয় পরিচয়: ১২৩৪৫৬৭৮৯০";
        let out = scrub_plain(input).text;
        assert!(
            !out.contains("০"),
            "Expected all Bengali digits to be redacted, but found trailing digit: {}",
            out
        );
        assert!(
            out.contains("[National ID Number]") || out.contains("[Bangladeshi National ID Number]"),
            "Expected national ID label for Bengali digits with keyword, but got: {}",
            out
        );
    }

    /// Tests that precomposed U+09DF (`য়`) is normalized to canonical decomposed
    /// U+09AF U+09BC (`য` + `়`) so it matches `id_formats.csv` context keywords.
    #[test]
    fn test_bengali_precomposed_keyword_normalization() {
        let input = "জাতী\u{09DF} পরিচ\u{09DF}: ৯৮৭৬৫৪৩২১০";
        let out = scrub_plain(input).text;
        assert!(
            out.contains("[National ID Number]") || out.contains("[Bangladeshi National ID Number]"),
            "Expected precomposed Bengali keyword to match canonical registry entry, got: {}",
            out
        );
        assert!(!out.contains("৯৮৭৬৫৪৩২১০"), "Expected digits to be redacted");
    }

    #[test]
    fn test_unicode_name_and_id_tokens() {
        // ASCII name and ASCII ID captured as before
        let ascii_input = "Full Name: Johnathan Doe\nPolicy No: ABC-98765-XYZ.";
        let ascii_out = scrub_plain(ascii_input).text;
        assert!(!ascii_out.contains("Johnathan"), "leaked ASCII name");
        assert!(!ascii_out.contains("ABC-98765-XYZ"), "leaked ASCII ID");
        assert!(ascii_out.contains("[NAME]"), "expected [NAME]");
        assert!(ascii_out.contains("[ID_NUMBER]"), "expected [ID_NUMBER]");

        // Bengali name value after an English label
        let bengali_input = "Full Name: অনিক আহমেদ";
        let bengali_out = scrub_plain(bengali_input).text;
        assert!(!bengali_out.contains("অনিক"), "leaked Bengali first name: {}", bengali_out);
        assert!(!bengali_out.contains("আহমেদ"), "leaked Bengali last name: {}", bengali_out);
        assert!(bengali_out.contains("[NAME]"), "expected [NAME] placeholder, got: {}", bengali_out);
    }

    #[test]
    fn test_label_at_line_end_redacts_name_on_next_line() {
        let out = scrub_plain("নাম\nঅনিক মজুমদার\nঅন্য লাইন").text;
        assert!(!out.contains("অনিক"), "name on next line leaked: {}", out);
        assert!(out.contains("[NAME]"), "expected [NAME], got: {}", out);
    }

    #[test]
    fn test_single_pipe_after_label_is_not_separator() {
        let out = scrub_plain("নাম | আজকে আবহাওয়া ভালো").text;
        assert!(out.contains("আজকে"), "single pipe must not redact next word: {}", out);
    }

    #[test]
    fn test_table_row_pipes_are_separator() {
        let out = scrub_plain("| নাম | রহিম উদ্দিন | ফোন |").text;
        assert!(!out.contains("রহিম"), "table-row value leaked: {}", out);
    }

    #[test]
    fn test_bangla_visarga_is_not_a_delimiter() {
        let tsv = "NAME\tbn\tdraft\tনাম\n";
        let g = parse_label_table(tsv).unwrap()[0].to_label_group().unwrap();
        assert!(g.re.find("নামঃ অনিক").is_none(), "visarga must not act as a separator");
    }

    #[test]
    fn test_accepted_over_redaction_function_word_after_naam() {
        // ACCEPTED OVER-REDACTION: the label rule redacts any letter token after নাম,
        // including the question particle কি. This asserts current behavior.
        let out = scrub_plain("তার নাম কি ।").text;
        assert!(!out.contains("কি"), "expected কি redacted (accepted over-redaction): {}", out);
    }

    // --- Cross-script label values: the label path (value_span -> NAME_TOKEN_RE, any
    // \p{L}\p{M} token) already redacts a value in any script; these tests lock that in.
    #[test]
    fn test_cross_script_bn_label_latin_value() {
        let out = scrub_plain("নাম: Zalmon Quibble").text;
        assert_eq!(out, "নাম: [NAME]");
    }

    #[test]
    fn test_cross_script_bn_label_han_value() {
        assert_eq!(scrub_plain("নাম: 田中太郎").text, "নাম: [NAME]");
    }

    #[test]
    fn test_cross_script_bn_label_hangul_value() {
        assert_eq!(scrub_plain("নাম: 김민준").text, "নাম: [NAME]");
    }

    #[test]
    fn test_cross_script_ja_label_bengali_value() {
        assert_eq!(scrub_plain("名前: নীলাঞ্জন").text, "名前: [NAME]");
    }

    #[test]
    fn test_cross_script_ko_label_bengali_value() {
        assert_eq!(scrub_plain("이름: নীলাঞ্জন").text, "이름: [NAME]");
    }

    #[test]
    fn test_cross_script_en_label_bengali_value() {
        assert_eq!(scrub_plain("Name: নীলাঞ্জন").text, "Name: [NAME]");
    }

    #[test]
    fn test_cross_script_es_label_bengali_value() {
        assert_eq!(scrub_plain("Nombre completo: নীলাঞ্জন").text, "Nombre completo: [NAME]");
    }

    #[test]
    fn test_cross_script_es_label_hangul_value() {
        assert_eq!(scrub_plain("Nombre completo: 김민준").text, "Nombre completo: [NAME]");
    }

    #[test]
    fn test_cross_script_ja_label_latin_value() {
        assert_eq!(scrub_plain("名前: Zalmon Quibble").text, "名前: [NAME]");
    }

    #[test]
    fn test_same_script_ja_label_ja_value_and_bn_label_bn_value() {
        assert_eq!(scrub_plain("名前: 田中太郎").text, "名前: [NAME]");
        assert_eq!(scrub_plain("নাম: অনিক মজুমদার").text, "নাম: [NAME]");
    }

    #[test]
    fn test_label_followed_by_digits_only_is_not_redacted_as_name() {
        assert_eq!(scrub_plain("নাম: 12345").text, "নাম: 12345");
    }

    #[test]
    fn test_bn_label_known_word_redacted_accepted_over_redaction() {
        // ACCEPTED OVER-REDACTION: the NAME label group skips the dictionary check, so a
        // known Bangla word after নাম is redacted. Asserts actual behavior.
        assert_eq!(scrub_plain("নাম: ভালো").text, "নাম: [NAME]");
    }

    // --- Foreign-run rule tests ---
    #[test]
    fn test_foreign_run_han_name_in_name_list_redacted() {
        // Bangla sentence with a Han name in the name list: redacted.
        let out = scrub_plain("আজ সকালে 田中 অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_han_token_not_in_any_list_block_redacted() {
        // Bangla sentence with a Han token not in any list: redacted (block of 3+ letters).
        let out = scrub_plain("আজ সকালে 未知者 未知者 অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_english_word_kept() {
        // Bangla sentence with an English word in the English word list: kept.
        let out = scrub_plain("আজ সকালে window অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে window অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_also_in_bangla_sentence_kept() {
        let out = scrub_plain("আজ সকালে also অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে also অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_close_in_bangla_sentence_kept() {
        let out = scrub_plain("আজ সকালে close অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে close অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_james_in_bangla_sentence_redacted() {
        let out = scrub_plain("আজ সকালে James অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_sara_in_bangla_sentence_redacted() {
        let out = scrub_plain("আজ সকালে Sara অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_lowercase_kim_in_bangla_sentence_redacted() {
        let out = scrub_plain("আজ সকালে kim অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_lowercase_john_in_bangla_sentence_redacted() {
        let out = scrub_plain("আজ সকালে john অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_joy_in_bangla_sentence_redacted_accepted_over_redaction() {
        // "Joy" is both an English common noun and a common given name retained in namelist.txt.
        // In a foreign run in Bangla text, matching the name list redacts it as an accepted over-redaction.
        let out = scrub_plain("আজ সকালে Joy অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_will_redacted_may_kept_accepted_over_redaction() {
        // "will" is an English modal verb retained in namelist.txt; in a foreign run in Bangla text,
        // matching the name list redacts it as an accepted over-redaction.
        // "may" is also retained in namelist.txt, but it is additionally present in NAME_ALLOWLIST
        // (as the calendar month May), which correctly takes precedence and keeps it unredacted.
        let out_will = scrub_plain("আজ সকালে will অফিসে এসেছিলেন").text;
        assert_eq!(out_will, "আজ সকালে [NAME] অফিসে এসেছিলেন");
        let out_may = scrub_plain("আজ সকালে may অফিসে এসেছিলেন").text;
        assert_eq!(out_may, "আজ সকালে may অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_art_more_close_in_bangla_sentence_kept() {
        // English words "art", "more", "close" are in FOREIGN_EN_WORDS and removed from namelist.txt.
        // In a Bangla sentence, they are recognized as English words and kept.
        let out = scrub_plain("আজ সকালে art এবং more এবং close অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে art এবং more এবং close অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_so_very_then_in_bangla_sentence_kept() {
        // English function words "so", "very", "then" are in FOREIGN_EN_WORDS and removed from namelist.txt.
        // In a Bangla sentence, they are recognized as English words and kept.
        let out = scrub_plain("আজ সকালে so এবং very এবং then অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে so এবং very এবং then অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_john_smith_in_bangla_sentence_redacted() {
        let out = scrub_plain("আজ সকালে John Smith অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_english_pipeline_john_handled_identically() {
        // English pipeline uses WORDS_FULL unchanged; verify baseline behavior on an English sentence.
        let out = scrub_plain("Yesterday John visited the office in London.").text;
        assert_eq!(out, "Yesterday [NAME] visited the office in London.");
    }

    #[test]
    fn test_foreign_run_n_not_equal_1_kept() {
        let out = scrub_plain("সূত্রের জন্য n≠1 প্রযোজ্য।").text;
        assert_eq!(out, "সূত্রের জন্য n≠1 প্রযোজ্য।");
    }

    #[test]
    fn test_foreign_run_m_not_equal_1_kept() {
        let out = scrub_plain("সূত্রের জন্য m≠1 প্রযোজ্য।").text;
        assert_eq!(out, "সূত্রের জন্য m≠1 প্রযোজ্য।");
    }

    #[test]
    fn test_foreign_run_n_not_equal_minus_1_kept() {
        let out = scrub_plain("সূত্রের জন্য n≠−1 প্রযোজ্য।").text;
        assert_eq!(out, "সূত্রের জন্য n≠−1 প্রযোজ্য।");
    }

    #[test]
    fn test_foreign_run_english_name_redacted() {
        // Bangla sentence with an English name in the English name list: redacted.
        let out = scrub_plain("আজ সকালে Arthur Dent অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_hiroaki_suffix_split_redacted() {
        // Bangla sentence with "Hiroaki-র": "Hiroaki" redacted, "-র" kept.
        let out = scrub_plain("আজ সকালে Hiroaki-র অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME]-র অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_single_latin_token_not_in_list_kept() {
        // Single Latin token in a Bangla sentence, not in any list: kept.
        let out = scrub_plain("আজ সকালে xyzzyfoo অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে xyzzyfoo অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_single_latin_token_in_name_list_redacted() {
        // Single Latin token in a Bangla sentence, in the English name list: redacted.
        let out = scrub_plain("আজ সকালে Alice অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_devanagari_block_redacted_as_other() {
        // Block with a Devanagari (Hindi) run in a Bangla document: redacted as "other".
        let out = scrub_plain("আজ সকালে नमस्ते भारत অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_foreign_run_ja_dominant_bengali_word_kept() {
        // Japanese-dominant line with a Bengali-script token in the Bengali word list: kept.
        let out = scrub_plain("先日の会議で ভালো について議論しました。").text;
        assert_eq!(out, "先日の会議で ভালো について議論しました。");
    }

    #[test]
    fn test_foreign_run_no_script_change_unchanged() {
        // Document with no script change: output unchanged.
        let out = scrub_plain("আজ সকালে আমরা অফিসে এসেছিলাম।").text;
        assert_eq!(out, "আজ সকালে আমরা অফিসে এসেছিলাম।");
    }

    // --- Romanized ja/ko/bn name lists (names/romanized/*.txt) in regular Bangla text ---
    #[test]
    fn test_macron_alias_and_original_redacted_in_bangla_text() {
        // Japanese surname with a macron and its ASCII alias inside Bangla text are both redacted.
        // "kōsaka" and its alias "kosaka" are in ja_surname.txt (neither in namelist.txt).
        assert_eq!(
            scrub_plain("আজ সকালে Kōsaka অফিসে এসেছিলেন").text,
            "আজ সকালে [NAME] অফিসে এসেছিলেন"
        );
        assert_eq!(
            scrub_plain("আজ সকালে kosaka অফিসে এসেছিলেন").text,
            "আজ সকালে [NAME] অফিসে এসেছিলেন"
        );
        // Also verify Kōsuke / kosuke
        assert_eq!(
            scrub_plain("আজ সকালে Kōsuke অফিসে এসেছিলেন").text,
            "আজ সকালে [NAME] অফিসে এসেছিলেন"
        );
        assert_eq!(
            scrub_plain("আজ সকালে kosuke অফিসে এসেছিলেন").text,
            "আজ সকালে [NAME] অফিসে এসেছিলেন"
        );
    }

    #[test]
    fn test_ko_given_names_with_spaces_not_present() {
        let content = read_manifest_file_string("ko", "ko_names_romanized.txt")
            .expect("read ko/ko_names_romanized.txt");
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            assert!(
                !trimmed.contains(' '),
                "Entry with space found in ko_names_romanized.txt: {:?}",
                trimmed
            );
        }
    }

    #[test]
    fn test_romanized_bn_name_only_in_bn_romanized_redacted() {
        // "abanindra" is only in bn_romanized.txt (not in en_names.txt).
        let out = scrub_plain("আজ সকালে abanindra অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_two_letter_forms_not_present_in_bn_romanized() {
        let content = read_manifest_file_string("bn", "bn_romanized.txt")
            .expect("read bn/bn_romanized.txt");
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            let letter_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
            assert!(
                letter_count > 2,
                "Entry with <=2 letters found in bn_romanized.txt: {:?}",
                trimmed
            );
        }
    }

    #[test]
    fn test_label_path_bangla_romanized_name_after_bangla_label_redacted() {
        assert_eq!(scrub_plain("নাম: abanindra").text, "নাম: [NAME]");
    }

    #[test]
    fn test_romanized_ja_surname_in_bangla_text_redacted() {
        // "adachibara" is only in ja_surnames_romanized.txt.
        let out = scrub_plain("আজ সকালে adachibara অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
    }

    #[test]
    fn test_romanized_ko_hyphenated_name_redacted_whole_token() {
        // "jong-su" is only in ko_names_romanized.txt; the whole hyphenated token must go.
        let out = scrub_plain("আজ সকালে jong-su অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে [NAME] অফিসে এসেছিলেন");
        assert!(!out.contains("-su") && !out.contains("jong"), "partial redaction: {}", out);
    }

    #[test]
    fn test_two_letter_latin_token_in_bangla_text_not_redacted() {
        let out = scrub_plain("আজ সকালে an অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে an অফিসে এসেছিলেন");
    }

    #[test]
    fn test_wordlist_only_latin_token_in_bangla_text_not_redacted() {
        // "window" is in en_words_full.txt and in no name list.
        let out = scrub_plain("আজ সকালে window অফিসে এসেছিলেন").text;
        assert_eq!(out, "আজ সকালে window অফিসে এসেছিলেন");
    }

    #[test]
    fn test_romanized_name_lists_are_not_in_namelist_txt() {
        // Separate, source-tagged files: the new entries must not have been merged into en_names.txt.
        let en_names = read_manifest_file_string("en", "en_names.txt").expect("read en/en_names.txt");
        let multilang = get_multilang_names();
        for n in ["adachibara", "jong-su", "abanindra"] {
            assert!(
                !en_names.lines().any(|l| l.trim() == n),
                "{} leaked into en_names.txt",
                n
            );
            assert!(multilang.contains_key(n), "{} missing from Latin name lookup", n);
        }
    }

    // --- Label path does not depend on the name lists or the length filter ---
    #[test]
    fn test_label_path_two_letter_token_after_bangla_label_redacted() {
        assert_eq!(scrub_plain("নাম: an").text, "নাম: [NAME]");
        assert_eq!(scrub_plain("নাম: mi").text, "নাম: [NAME]");
    }

    #[test]
    fn test_label_path_latin_name_after_japanese_label_redacted() {
        assert_eq!(scrub_plain("名前: masabumi").text, "名前: [NAME]");
    }

    #[test]
    fn test_label_table_english_identical() {
        // 1. Verify compiled regex source strings match expected
        // Alternatives are unchanged; only the shared separator (LABEL_SEPARATOR) gained
        // newline and pipe support.
        assert_eq!(
            LABEL_GROUPS[0].re.as_str(),
            format!(r"(?i)\b(?:emergency\s+contact\s+name|father['’]?s\s+name|mother['’]?s\s+name|spouse['’]?s\s+name){}", LABEL_SEPARATOR)
        );
        assert_eq!(
            LABEL_GROUPS[1].re.as_str(),
            format!(r"(?i)\b(?:holder['’]?s\s+name|given\s+names?|sur\s?name|full\s+name|first\s+name|last\s+name|name){}", LABEL_SEPARATOR)
        );
        assert_eq!(
            LABEL_GROUPS[2].re.as_str(),
            format!(r"(?i)\b(?:permanent\s+address|present\s+address|mailing\s+address|address|residence|street){}", LABEL_SEPARATOR)
        );

        // 2. Fixed set of invented English inputs
        let samples = [
            ("Emergency Contact Name: Arthur Dent", "Emergency Contact Name: [NAME]"),
            ("Full Name: Arthur Dent", "Full Name: [NAME]"),
            ("Address: 42 Wallaby Way, Sydney", "Address: [ADDRESS]"),
            ("Date of Birth: 12/25/1990", "Date of Birth: [DATE]"),
            ("Policy No: POL-998877", "Policy No: [ID_NUMBER]"),
            ("Telephone: 555-123-4567", "Telephone: [PHONE]"),
            ("Nationality: Canadian", "Nationality: [PLACE]"),
        ];
        for (input, expected) in samples {
            let out = scrub_plain(input).text;
            assert_eq!(out, expected, "Mismatch for input: {}", input);
        }
    }

    #[test]
    fn test_label_table_new_language_entry() {
        // Test-only label in an unreviewed language section
        let test_tsv = "NAME\tbn\tdraft\tনাম\n";
        let parsed = parse_label_table(test_tsv).expect("valid test TSV");
        assert_eq!(parsed.len(), 1);
        let group = parsed[0].to_label_group().expect("valid group");
        let mut unreviewed = std::collections::BTreeSet::new();
        let out = apply_label_group(
            "নাম: অনিক আহমেদ",
            &group,
            &ScrubOptions::without_identity(),
            &mut unreviewed,
        );
        assert!(out.contains("[NAME]"), "expected [NAME], got: {}", out);
        assert!(!out.contains("অনিক"), "leaked Bengali name value");
        assert!(
            unreviewed.contains("bn"),
            "expected unreviewed language 'bn' to be recorded in report"
        );
    }

    #[test]
    fn test_malformed_label_table_returns_clear_error() {
        // 1. Missing fields
        let res = validate_label_table("NAME\ten\treviewed\n");
        assert!(res.is_err(), "Expected error on missing fields");

        // 2. Empty alternative
        let res = validate_label_table("NAME\ten\treviewed\t\n");
        assert!(res.is_err(), "Expected error on empty alternative");

        // 3. Trailing empty alternative
        let res = validate_label_table("NAME\ten\treviewed\tfirst name\t\n");
        assert!(res.is_err(), "Expected error on trailing empty alternative");

        // 4. Invalid status
        let res = validate_label_table("NAME\ten\tactive\tfirst name\n");
        assert!(res.is_err(), "Expected error on invalid status");

        // 5. Unknown group
        let res = validate_label_table("CUSTOM\ten\treviewed\tfirst name\n");
        assert!(res.is_err(), "Expected error on unknown group");

        // 6. Invalid regex syntax
        let res = validate_label_table("NAME\ten\treviewed\t(unclosed regex\n");
        assert!(res.is_err(), "Expected error on invalid regex");

        // 7. Verify current LABEL_TSV is valid
        let res = validate_label_table(LABEL_TSV);
        assert!(res.is_ok(), "Expected current LABEL_TSV to be valid: {:?}", res.err());
    }

    #[test]
    #[ignore = "Accepted trade-off: line-bounded label capture over-redacts same-line text. Revisit with real OCR samples."]
    fn test_id_label_value_followed_by_ordinary_sentence_not_redacted() {
        let input = "Reference No: ZX-98741-K. The team finalized the logistics plan for the quarterly shipment.";
        let out = scrub_plain(input).text;
        assert!(!out.contains("ZX-98741-K"), "ID value should be redacted: {}", out);
        assert!(out.contains("[ID_NUMBER]"), "Expected [ID_NUMBER] tag: {}", out);
        assert!(
            out.contains("The team finalized the logistics plan for the quarterly shipment."),
            "Sentence after ID was erroneously redacted: {}",
            out
        );
    }

    #[test]
    #[ignore = "Accepted trade-off: line-bounded label capture over-redacts same-line text. Revisit with real OCR samples."]
    fn test_name_label_placeholder_followed_by_capitalized_phrase_over_redaction() {
        let input = "Full Name: [ID_NUMBER] Contoso Global Logistics";
        let out = scrub_plain(input).text;
        assert!(
            out.contains("Contoso Global Logistics"),
            "Over-redaction detected: capitalized phrase after placeholder was redacted as name: {}",
            out
        );
    }

    #[test]
    fn test_id_value_newline_sentence_not_redacted() {
        let input = "Passport No: A12345678\nThe team finalized the logistics plan for the quarterly shipment.";
        let out = scrub_plain(input).text;
        assert!(!out.contains("A12345678"), "ID value should be redacted: {}", out);
        assert!(out.contains("[ID_NUMBER]"), "Expected [ID_NUMBER] tag: {}", out);
        assert!(
            out.contains("The team finalized the logistics plan for the quarterly shipment."),
            "Sentence on next line was erroneously redacted: {}",
            out
        );
    }

    /// Accepted trade-off under line-bounded capture: recall is preferred over context on the same line.
    /// Ordinary sentences sharing the same line with an identifier value are redacted to prevent name leaks.
    #[test]
    fn test_same_line_text_after_id_is_redacted_by_design() {
        let input = "Account No: AC-44821-X. All quarterly financial disclosures have been uploaded.";
        let out = scrub_plain(input).text;
        assert!(!out.contains("AC-44821-X"), "ID value should be redacted: {}", out);
        assert!(
            !out.contains("All quarterly financial disclosures have been uploaded"),
            "Expected same-line text to be redacted by design: {}",
            out
        );
        assert!(out.contains("[ID_NUMBER]"), "Expected [ID_NUMBER] tag: {}", out);
    }

    #[test]
    fn test_devanagari_combining_vowels_tokenization() {
        // Devanagari values with combining vowel signs tokenize as a single token per word
        let input = "पूरा नाम: सिद्धार्थ देशमुख";
        let out = scrub_plain(input).text;
        assert!(!out.contains("सिद्धार्थ"), "Devanagari first name leaked: {}", out);
        assert!(!out.contains("देशमुख"), "Devanagari last name leaked: {}", out);
        assert_eq!(out, "पूरा नाम: [NAME]");
    }

    #[test]
    fn test_korean_hangul_space_separated_name() {
        // Space-separated Hangul values work with the existing token logic
        let input = "성명: 김 민준";
        let out = scrub_plain(input).text;
        assert!(!out.contains("김"), "Korean surname leaked: {}", out);
        assert!(!out.contains("민준"), "Korean given name leaked: {}", out);
        assert_eq!(out, "성명: [NAME]");
    }

    #[test]
    fn test_latin_accented_names_captured() {
        // Confirm accented Latin names in Spanish, French, and German are captured
        let input_es = "Nombre: José García";
        assert_eq!(scrub_plain(input_es).text, "Nombre: [NAME]");

        let input_fr = "Nom: Éloïse Müller";
        assert_eq!(scrub_plain(input_fr).text, "Nom: [NAME]");

        let input_de = "Name: Jörg Weiß";
        assert_eq!(scrub_plain(input_de).text, "Name: [NAME]");
    }

    #[test]
    fn test_japanese_unspaced_scanner_behavior() {
        // Japanese bounded rule stops at particles such as は, capturing the name without eating the clause
        let input = "氏名: 山田太郎は技術者です";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]は技術者です");
    }

    #[test]
    fn test_japanese_bounded_name_rule() {
        // The sample name is redacted and the following clause is not
        let input = "氏名: 山田太郎は技術者です。";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]は技術者です。");

        // Invented sample stopping at punctuation
        let input_punct = "氏名: 佐藤一郎。その他の情報";
        let out_punct = scrub_plain(input_punct).text;
        assert_eq!(out_punct, "氏名: [NAME]。その他の情報");
    }

    #[test]
    fn test_japanese_bounded_address_rule() {
        // Invented Japanese address fully redacted up to punctuation, following clause preserved
        let input = "住所: 青森県架空市中央町9-9-9。次の文はそのまま残ります。";
        let out = scrub_plain(input).text;
        assert_eq!(out, "住所: [ADDRESS]。次の文はそのまま残ります。");
    }

    #[test]
    fn test_multilingual_single_date_tag() {
        // Japanese date with label: single date, single tag (no duplicate [DATE][DATE])
        let input_ja = "日付: 2024-05-01。";
        let out_ja = scrub_plain(input_ja).text;
        assert_eq!(out_ja, "日付: [DATE]。");

        // Hindi date with label: single date, single tag
        let input_hi = "तारीख: 15/08/1995।";
        let out_hi = scrub_plain(input_hi).text;
        assert_eq!(out_hi, "तारीख: [DATE]।");
    }

    #[test]
    fn test_lowercase_latin_name_redacted() {
        // Lowercase Latin name without label must be captured by script-neutral name lookup
        let input = "The package was dispatched to mateo yesterday.";
        let rep = scrub_plain(input);
        assert_eq!(rep.text, "The package was dispatched to [NAME] yesterday.");
        assert!(rep.matched_name_languages.contains("es"));
        assert!(rep.unreviewed_languages.contains("es"));
    }

    #[test]
    fn test_bengali_name_redacted() {
        // Unscripted / non-Latin name without case distinction must be captured
        let input = "গতকাল অনিক একটি নতুন বই কিনেছেন।";
        let rep = scrub_plain(input);
        assert_eq!(rep.text, "গতকাল [NAME] একটি নতুন বই কিনেছেন।");
        assert!(rep.matched_name_languages.contains("bn"));
        assert!(rep.unreviewed_languages.contains("bn"));
    }

    #[test]
    fn test_ordinary_word_matching_name_not_redacted_without_label() {
        // "mark" is in namelist.txt but is an ordinary English dictionary word
        // It must NOT be redacted when appearing in an ordinary sentence without a name label
        let input = "Please mark the shipping container as inspected.";
        let rep = scrub_plain(input);
        assert_eq!(rep.text, "Please mark the shipping container as inspected.");
        assert!(!rep.matched_name_languages.contains("en"));
    }

    #[test]
    fn test_lowercase_list_name_in_unlabeled_sentence_redacted() {
        // Invented sentence containing lowercase names from the multilingual name lists without any label
        let input = "The invoice was signed by julien and forwarded to lukas yesterday.";
        let rep = scrub_plain(input);
        assert_eq!(
            rep.text,
            "The invoice was signed by [NAME] and forwarded to [NAME] yesterday."
        );
        assert!(rep.matched_name_languages.contains("fr"));
        assert!(rep.matched_name_languages.contains("de"));
    }

    #[test]
    fn test_sentence_beginning_with_common_preposition_not_over_redacted() {
        // Sentences starting with prepositions like "De" or "A" must not trigger cue-based name redaction
        let input_de = "De la part du service administratif, nous confirmons la réception.";
        let out_de = scrub_plain(input_de).text;
        assert_eq!(out_de, input_de);

        let input_a = "A tutti i partecipanti della conferenza, confermiamo l'orario di inizio.";
        let out_a = scrub_plain(input_a).text;
        assert_eq!(out_a, input_a);
    }

    #[test]
    fn test_allowlist_company_term_in_sentence_not_redacted() {
        let input = "We deployed our service to Microsoft and Google cloud.";
        let out = scrub_plain(input).text;
        assert_eq!(out, input);
    }

    #[test]
    fn test_allowlist_does_not_suppress_containing_term() {
        // Dockerson contains allowlisted "docker", but must not be suppressed if it's a name candidate
        let input = "The agreement was signed by Dockerson yesterday.";
        let out = scrub_plain(input).text;
        assert!(
            out.contains("[NAME]"),
            "Expected Dockerson to be redacted as unknown caps, got: {}",
            out
        );
    }

    #[test]
    fn test_identity_token_in_allowlist_still_redacted() {
        // User's own registered name is "Docker", which is also in allowlist.txt
        let mut opts = ScrubOptions::without_identity();
        opts.identity_tokens = vec!["docker".to_string()];
        let input = "Please contact Docker for the project update.";
        let rep = scrub_text_with(input, &opts);
        assert_eq!(rep.text, "Please contact [NAME] for the project update.");
    }

    #[test]
    fn test_label_anchored_value_in_allowlist_still_redacted() {
        // Value following a label that appears in allowlist must still be redacted
        let input = "Full Name: Docker";
        let out = scrub_plain(input).text;
        assert_eq!(out, "Full Name: [NAME]");
    }

    #[test]
    fn test_name_label_dictionary_word_unknown_redacted() {
        let input = "Full Name: Unknown";
        let out = scrub_plain(input).text;
        assert_eq!(out, "Full Name: [NAME]");
    }

    #[test]
    fn test_salutation_edge_window_first_line_counts() {
        let input = "Dear Arthur,\nLine 2.\nLine 3.\nLine 4.\nLine 5.\nLine 6.\nLine 7.\nLine 8.\n";
        let out = scrub_plain(input).text;
        assert!(out.starts_with("Dear [NAME],"), "Salutation on first line should be redacted, got: {}", out);
    }

    #[test]
    fn test_salutation_edge_window_middle_ignored() {
        let input = "Line 1.\nLine 2.\nLine 3.\nWe send thanks to Field Operations for the update.\nLine 5.\nLine 6.\nLine 7.\nLine 8.\n";
        let out = scrub_salutation_cues(input, &ScrubOptions::without_identity());
        assert!(!out.contains("[NAME]"), "Mid-document salutation cue should be ignored, got: {}", out);
        assert!(out.contains("thanks to Field Operations"));
    }

    #[test]
    fn test_salutation_edge_window_short_document_fully_inside() {
        let input = "Line 1.\nLine 2.\nDear Arthur,\nLine 4.\nLine 5.\n";
        let out = scrub_salutation_cues(input, &ScrubOptions::without_identity());
        assert!(out.contains("Dear [NAME]"), "Salutation in short doc should be redacted, got: {}", out);
    }

    #[test]
    fn test_duplicate_row_merge_at_load_time() {
        let tsv = "NAME\ten\tdraft\tjohn\tjane\nNAME\ten\tdraft\tjane\tbob\n";
        let parsed = parse_label_table(tsv).expect("duplicate rows should merge cleanly");
        assert_eq!(parsed.len(), 1, "Duplicate (GROUP, LANG) rows should be collapsed into one");
        assert_eq!(parsed[0].group_name, "NAME");
        assert_eq!(parsed[0].lang, "en");
        assert_eq!(parsed[0].status, "draft");
        assert_eq!(parsed[0].alternatives, vec!["john", "jane", "bob"]);
    }

    #[test]
    fn test_mid_text_miss_and_best_no_name_tag() {
        let input = "Line 1.\nLine 2.\nLine 3.\nI miss London and we chose the best Option.\nLine 5.\nLine 6.\nLine 7.\nLine 8.\n";
        let out = scrub_plain(input).text;
        assert!(!out.contains("[NAME]"), "Mid-text 'miss London' and 'best Option' should not produce [NAME], got: {}", out);
        assert!(out.contains("miss London"));
        assert!(out.contains("best Option"));
    }

    #[test]
    fn test_salutation_always_on_dear_maria_line_40() {
        let mut lines = Vec::new();
        for i in 1..=50 {
            if i == 40 {
                lines.push("Dear Maria, thank you for writing.".to_string());
            } else {
                lines.push(format!("This is line number {} of the report.", i));
            }
        }
        let input = lines.join("\n");
        let out = scrub_plain(&input).text;
        assert!(out.contains("Dear [NAME],"), "Dear Maria on line 40 should redact Maria, got: {}", out);
        assert!(!out.contains("Maria"), "Maria should not leak");
    }

    #[test]
    fn test_salutation_edge_gated_regards_line_2_vs_line_20() {
        let mut lines = Vec::new();
        for i in 1..=50 {
            if i == 2 {
                lines.push("Regards Alice,".to_string());
            } else if i == 20 {
                lines.push("We send our warm regards to the department.".to_string());
            } else {
                lines.push(format!("This is line number {} of the memo.", i));
            }
        }
        let input = lines.join("\n");
        let out = scrub_plain(&input).text;
        assert!(out.contains("Regards [NAME],"), "Regards on line 2 should redact Alice, got: {}", out);
        assert!(!out.contains("Alice"), "Alice should not leak");
        assert!(out.contains("warm regards to the department"), "Regards on line 20 should not produce [NAME], got: {}", out);
    }

    #[test]
    fn test_multitoken_name_with_particle() {
        let input = "Name: Ana van der Berg\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "Name: [NAME]\n");
        assert!(!out.contains("Ana"));
        assert!(!out.contains("van"));
        assert!(!out.contains("der"));
        assert!(!out.contains("Berg"));
    }

    #[test]
    fn test_multitoken_name_split_across_line_break() {
        let input = "Name: John\nSmith\n";
        let out = scrub_plain(input).text;
        assert!(out.contains("Name: [NAME]\n"));
        assert!(!out.contains("John"));
        assert!(!out.contains("Smith"));

        let input_particle = "Name: Ana van\nder Berg\n";
        let out_particle = scrub_plain(input_particle).text;
        assert!(out_particle.contains("Name: [NAME]\n"));
        assert!(!out_particle.contains("Ana"));
        assert!(!out_particle.contains("van"));
        assert!(!out_particle.contains("der"));
        assert!(!out_particle.contains("Berg"));
    }

    #[test]
    fn test_multitoken_name_long_japanese_near_span_cap() {
        // 6 kanji characters: 長谷川健太郎
        let input = "氏名: 長谷川健太郎\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]\n");
        assert!(!out.contains("長谷川健太郎"));
        assert!(!out.contains("長"));
        assert!(!out.contains("健"));
    }

    #[test]
    fn test_japanese_name_cap_7_characters() {
        // 7 kanji characters: 長谷川健太郎一
        let input = "氏名: 長谷川健太郎一\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]\n");
        assert!(!out.contains("長谷川健太郎一"));
    }

    #[test]
    fn test_japanese_name_cap_8_characters() {
        // 8 kanji characters: 長谷川健太郎一郎
        let input = "氏名: 長谷川健太郎一郎\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]\n");
        assert!(!out.contains("長谷川健太郎一郎"));
    }

    #[test]
    fn test_japanese_name_cap_10_characters() {
        // 10 kanji characters: 長谷川健太郎一郎次郎
        let input = "氏名: 長谷川健太郎一郎次郎\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]\n");
        assert!(!out.contains("長谷川健太郎一郎次郎"));
    }

    #[test]
    fn test_japanese_name_cap_11_characters() {
        // 11 kanji characters: 長谷川健太郎一郎次郎三
        let input = "氏名: 長谷川健太郎一郎次郎三\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]\n");
        assert!(!out.contains("長谷川健太郎一郎次郎三"));
    }

    #[test]
    fn test_japanese_name_cap_12_characters() {
        // 12 kanji characters: 長谷川健太郎一郎次郎三郎
        let input = "氏名: 長谷川健太郎一郎次郎三郎\n";
        let out = scrub_plain(input).text;
        assert_eq!(out, "氏名: [NAME]\n");
        assert!(!out.contains("長谷川健太郎一郎次郎三郎"));
    }

    #[test]
    fn test_multitoken_name_followed_by_comma_or_parenthesis() {
        let input_comma = "Name: John Smith, Manager\n";
        let out_comma = scrub_plain(input_comma).text;
        assert!(!out_comma.contains("John"), "John should be redacted");
        assert!(!out_comma.contains("Smith"), "Smith should be redacted");

        let input_paren = "Name: John Smith (CEO)\n";
        let out_paren = scrub_plain(input_paren).text;
        assert!(out_paren.starts_with("Name: [NAME] (CEO)"));
        assert!(!out_paren.contains("John"), "John should be redacted");
        assert!(!out_paren.contains("Smith"), "Smith should be redacted");
    }

    #[test]
    fn test_japanese_context_rules() {
        // Pair in prose: 佐藤花子 (surname + given name). Expect redaction.
        let pair_in_prose = "会議の進行は佐藤花子が担当しました。";
        let out_pair = scrub_plain(pair_in_prose).text;
        assert!(!out_pair.contains("佐藤花子"), "佐藤花子 should be redacted: {}", out_pair);
        assert!(!out_pair.contains("佐藤"), "佐藤 should be redacted: {}", out_pair);
        assert!(!out_pair.contains("花子"), "花子 should be redacted: {}", out_pair);

        // Honorific: 田中さん. Expect redaction of the name only.
        let honorific_san = "田中さん、明日の午前十時からの打ち合わせ資料です。";
        let out_san = scrub_plain(honorific_san).text;
        assert!(!out_san.contains("田中"), "田中 should be redacted: {}", out_san);
        assert!(out_san.contains("さん"), "さん should remain unredacted: {}", out_san);

        // Honorific: 佐藤部長. Expect redaction of the name only.
        let honorific_bucho = "本件については佐藤部長に直接ご相談ください。";
        let out_bucho = scrub_plain(honorific_bucho).text;
        assert!(!out_bucho.contains("佐藤"), "佐藤 should be redacted: {}", out_bucho);
        assert!(out_bucho.contains("部長"), "部長 should remain unredacted: {}", out_bucho);

        // English context: a Japanese name between English words. Expect redaction.
        let en_ctx = "The document was signed by だいすけ yesterday.";
        let out_en = scrub_plain(en_ctx).text;
        assert!(!out_en.contains("だいすけ"), "だいすけ in English context should be redacted: {}", out_en);

        let en_ctx_sur = "Please send the file to 佐藤 today.";
        let out_en_sur = scrub_plain(en_ctx_sur).text;
        assert!(!out_en_sur.contains("佐藤"), "佐藤 in English context should be redacted: {}", out_en_sur);

        // Lone given name in Japanese prose with no cue (followed by に). Expect no redaction.
        let lone_prose = "来週のスケジュールについてだいすけに連絡してください。";
        let out_lone = scrub_plain(lone_prose).text;
        assert!(out_lone.contains("だいすけ"), "lone given name with no cue in Japanese prose should NOT be redacted: {}", out_lone);

        // Labeled field with a lone given name. Expect redaction.
        let labeled_lone = "名前: 宗賢\n";
        let out_labeled = scrub_plain(labeled_lone).text;
        assert_eq!(out_labeled, "名前: [NAME]\n");
        assert!(!out_labeled.contains("宗賢"), "labeled lone given name should be redacted: {}", out_labeled);
    }

    #[test]
    fn test_japanese_repeated_mention_same_call_only() {
        // Multi-sentence input: first sentence mentions full name 佐藤花子 (redacted via Rule 2a),
        // middle sentence has no names, third sentence has lone 花子 with particle に (not a subject particle).
        let same_call = "佐藤花子が出席しました。会議では新しい企画が議論されました。その後に花子に資料を渡しました。";
        let out_same = scrub_plain(same_call).text;
        assert!(
            !out_same.contains("花子"),
            "Later lone given name in a multi-sentence input in the same call should be redacted via repeated-mention cue: {}",
            out_same
        );
        assert!(
            out_same.contains("その後に[NAME]に資料を渡しました。"),
            "Expected third sentence lone given name to be replaced by [NAME]: {}",
            out_same
        );

        // In a separate call to scrub_text_with, lone 花子 with particle に must NOT redact.
        let different_call = "会議では新しい企画が議論されました。その後に花子に資料を渡しました。";
        let out_diff = scrub_plain(different_call).text;
        assert!(
            out_diff.contains("花子"),
            "Lone given name in a different call without cues must NOT be redacted: {}",
            out_diff
        );
    }

    #[test]
    fn test_japanese_single_char_surname_rules() {
        // Single-character surnames tagged as 姓 by Lindera (林, 森) paired with a given name redact.
        let hayashi = "本日の会議には林太郎が参加しました。";
        let out_hayashi = scrub_plain(hayashi).text;
        assert!(
            !out_hayashi.contains("林") && !out_hayashi.contains("太郎"),
            "林太郎 should redact completely: {}",
            out_hayashi
        );

        let mori = "担当の森花子に書類を送付しました。";
        let out_mori = scrub_plain(mori).text;
        assert!(
            !out_mori.contains("森") && !out_mori.contains("花子"),
            "森花子 should redact completely: {}",
            out_mori
        );

        // Ordinary prose phrases 何とも and 丸るい must NOT redact.
        let nantomo = "子供は何とも云わなかった。";
        let out_nantomo = scrub_plain(nantomo).text;
        assert_eq!(
            out_nantomo, nantomo,
            "何とも in ordinary prose must not redact: {}",
            out_nantomo
        );

        let marui = "柳の枝が丸るい影を往来の中へ落す。";
        let out_marui = scrub_plain(marui).text;
        assert_eq!(
            out_marui, marui,
            "丸るい in ordinary prose must not redact: {}",
            out_marui
        );
    }

    #[test]
    fn test_japanese_akari_common_noun_not_redacted() {
        let akari_prose = "信号標のあかりは汽車が通るときだけつくのだと云いました。";
        let out = scrub_plain(akari_prose).text;
        assert_eq!(
            out, akari_prose,
            "Common noun あかり in ordinary prose must not be redacted: {}",
            out
        );
    }

    #[test]
    fn test_japanese_title_before_name_cue() {
        // Title + optional の + surname/given name -> redacts the name only, leaving the title intact.
        let kacho_yamato = "課長の大和が説明しました";
        let out_yamato = scrub_plain(kacho_yamato).text;
        assert_eq!(
            out_yamato, "課長の[NAME]が説明しました",
            "Expected 大和 to be redacted after 課長の while preserving title: {}",
            out_yamato
        );

        let bucho_tanaka = "部長の田中が承認しました。";
        let out_tanaka = scrub_plain(bucho_tanaka).text;
        assert_eq!(
            out_tanaka, "部長の[NAME]が承認しました。",
            "Expected 田中 to be redacted after 部長の while preserving title: {}",
            out_tanaka
        );

        let kacho_direct = "課長大和が説明しました。";
        let out_direct = scrub_plain(kacho_direct).text;
        assert_eq!(
            out_direct, "課長[NAME]が説明しました。",
            "Expected 大和 to be redacted directly after 課長 while preserving title: {}",
            out_direct
        );

        // Title followed by non-name common words must NOT redact anything.
        let kacho_kaigi = "課長会議で報告";
        assert_eq!(
            scrub_plain(kacho_kaigi).text, kacho_kaigi,
            "Expected no redaction in 課長会議で報告"
        );

        let sensei_common = "先生は黒板の前に立って今日の予定を説明しました。";
        assert_eq!(
            scrub_plain(sensei_common).text, sensei_common,
            "Expected ordinary text with 先生 followed by common words to stay unredacted"
        );
    }

    #[test]
    fn test_japanese_subject_particle_cue() {
        let hanako_wa = "花子は新しい企画書を作成しました。";
        let out_hanako = scrub_plain(hanako_wa).text;
        assert!(
            !out_hanako.contains("花子"),
            "花子は should redact 花子 via subject-particle cue: {}",
            out_hanako
        );
        assert!(
            out_hanako.starts_with("[NAME]は"),
            "Particle は should remain intact: {}",
            out_hanako
        );

        let taro_ga = "太郎が午前の会議で発表しました。";
        let out_taro = scrub_plain(taro_ga).text;
        assert!(
            !out_taro.contains("太郎"),
            "太郎が should redact 太郎 via subject-particle cue: {}",
            out_taro
        );
        assert!(
            out_taro.starts_with("[NAME]が"),
            "Particle が should remain intact: {}",
            out_taro
        );
    }

    #[test]
    fn test_japanese_lone_given_name_without_cue_not_redacted() {
        // Documents the known gap: lone given name not preceded by full-name mention and not followed by は/が/も
        for (input, name) in [
            ("来週のスケジュールについてだいすけに連絡してください。", "だいすけ"),
            ("契約書の最終確認を米次郎に依頼しました。", "米次郎"),
            ("先日のセミナーで喜八郎の講演を聴講しました。", "喜八郎"),
        ] {
            let out = scrub_plain(input).text;
            assert!(
                out.contains(name),
                "Expected lone given name {} without cue to remain unredacted, got: {}",
                name,
                out
            );
        }
    }

    #[test]
    fn test_japanese_labeled_fields_always_redact() {
        for (input, expected, leaked) in [
            ("名前: だいすけ\n", "名前: [NAME]\n", "だいすけ"),
            ("氏名: 平八郎\n", "氏名: [NAME]\n", "平八郎"),
            ("お名前: カレン\n", "お名前: [NAME]\n", "カレン"),
            ("姓: 佐藤\n", "姓: [NAME]\n", "佐藤"),
            ("名: 花子\n", "名: [NAME]\n", "花子"),
        ] {
            let out = scrub_plain(input).text;
            assert_eq!(out, expected);
            assert!(!out.contains(leaked));
        }
    }

    #[test]
    fn test_japanese_cues_do_not_change_korean_or_latin() {
        // Korean behavior unchanged
        let kr = "김민씨는 이번 프로젝트 일정을 공유했습니다.";
        let out_kr = scrub_plain(kr).text;
        assert!(!out_kr.contains("김민"), "Korean name should still redact: {}", out_kr);

        // Latin behavior unchanged: ordinary English words that are also names ("will", "may", "mark")
        // in lowercase running text do not redact, while capitalized names mid-sentence redact.
        let en_clean = "We will mark the items that may require review.";
        assert_eq!(scrub_plain(en_clean).text, en_clean);

        let en_name = "We received a detailed proposal from Hiroaki this morning.";
        let out_en = scrub_plain(en_name).text;
        assert!(!out_en.contains("Hiroaki"), "Romanized name should still redact: {}", out_en);
    }

    #[test]
    fn test_korean_particles_and_honorifics() {
        let t1 = "김민씨는 이번 프로젝트 일정을 공유했습니다.";
        let out1 = scrub_plain(t1).text;
        assert!(!out1.contains("김민"), "김민 leaked: {}", out1);

        let t2 = "서준이가 발표 자료를 준비했습니다.";
        let out2 = scrub_plain(t2).text;
        assert!(!out2.contains("서준"), "서준 leaked: {}", out2);

        let t3 = "김민에게 주요 보고서를 전달했습니다.";
        let out3 = scrub_plain(t3).text;
        assert!(!out3.contains("김민"), "김민 leaked: {}", out3);

        let t4 = "김민은 내일 오전 회의에 참석합니다.";
        let out4 = scrub_plain(t4).text;
        assert!(!out4.contains("김민"), "김민 leaked: {}", out4);

        let t5 = "김민이 제안서를 최종 검토했습니다.";
        let out5 = scrub_plain(t5).text;
        assert!(!out5.contains("김민"), "김민 leaked: {}", out5);
    }

    #[test]
    fn test_korean_form_fields() {
        assert_eq!(scrub_plain("이름: 김민").text, "이름: [NAME]");
        assert_eq!(scrub_plain("성명: 김민준").text, "성명: [NAME]");
        assert_eq!(scrub_plain("이름: 서준").text, "이름: [NAME]");
        assert_eq!(scrub_plain("성함: 도윤").text, "성함: [NAME]");
        assert_eq!(scrub_plain("성명: 박지호").text, "성명: [NAME]");
        assert_eq!(scrub_plain("성: 김").text, "성: [NAME]");
        assert_eq!(scrub_plain("명: 민수").text, "명: [NAME]");
    }

    #[test]
    fn test_spanish_positive_cases() {
        // 1. Capitalized name mid-sentence
        let s1 = scrub_plain("Ayer hablé con Alejandro sobre el proyecto.").text;
        assert!(!s1.contains("Alejandro"), "Mid-sentence Spanish name leaked: {}", s1);

        // 2. Sentence-initial full name
        let s2 = scrub_plain("Mateo García presentó el informe anual ayer.").text;
        assert!(!s2.contains("Mateo") && !s2.contains("García"), "Sentence-initial Spanish name leaked: {}", s2);

        // 3. Multi-word surname with "de la"
        let s3 = scrub_plain("El documento fue firmado por Rodrigo de la Cruz en la reunión.").text;
        assert_eq!(s3, "El documento fue firmado por [NAME] en la reunión.");

        // 4. Multi-word surname with "del"
        let s4 = scrub_plain("La solicitud de María del Carmen fue aprobada hoy.").text;
        assert_eq!(s4, "La solicitud de [NAME] fue aprobada hoy.");

        // 5. Multi-word surname with "de"
        let s5 = scrub_plain("Consultamos el expediente con Miguel de Cervantes esta mañana.").text;
        assert_eq!(s5, "Consultamos el expediente con [NAME] esta mañana.");

        // 6. Multi-word surname with "y"
        let s6 = scrub_plain("El ensayo de José Ortega y Gasset fue citado en la conferencia.").text;
        assert_eq!(s6, "El ensayo de [NAME] fue citado en la conferencia.");

        // 7. Double surname ("García Márquez" style) as a full phrase
        let s7 = scrub_plain("La novela de Gabriel García Márquez está en la biblioteca.").text;
        assert_eq!(s7, "La novela de [NAME] está en la biblioteca.");

        // 8. Accented name and ñ (exact form)
        let s8 = scrub_plain("Enviamos el contrato a Íñigo Núñez por correo.").text;
        assert_eq!(s8, "Enviamos el contrato a [NAME] por correo.");

        // 9. Accent-stripped name form
        let s9 = scrub_plain("Enviamos el contrato a Inigo Nunez por correo.").text;
        assert_eq!(s9, "Enviamos el contrato a [NAME] por correo.");

        // 10. Titles: Sr., Sra., Srta., Dr., Dra., Lic., Ing. (title not redacted, name redacted)
        for (input, expected) in [
            ("El Sr. Carlos Rodríguez asistirá a la junta.", "El Sr. [NAME] asistirá a la junta."),
            ("La Sra. Elena Gómez confirmó su asistencia.", "La Sra. [NAME] confirmó su asistencia."),
            ("La Srta. Lucía Fernández entregó el reporte.", "La Srta. [NAME] entregó el reporte."),
            ("El Dr. Javier Morales revisó al paciente.", "El Dr. [NAME] revisó al paciente."),
            ("La Dra. Begoña López coordinó el estudio.", "La Dra. [NAME] coordinó el estudio."),
            ("El Lic. Fernando Sánchez redactó el acuerdo.", "El Lic. [NAME] redactó el acuerdo."),
            ("El Ing. Álvaro Torres supervisó la obra.", "El Ing. [NAME] supervisó la obra."),
        ] {
            let out = scrub_plain(input).text;
            assert_eq!(out, expected, "Failed title test for input: {}", input);
        }

        // 11. Ambiguous common-noun surnames when activated by a real person name neighbor
        let p_caballero = scrub_plain("El informe de José Caballero fue aprobado hoy.").text;
        assert_eq!(p_caballero, "El informe de [NAME] fue aprobado hoy.");

        let p_rey = scrub_plain("La decisión de Pedro Rey fue confirmada por el comité.").text;
        assert_eq!(p_rey, "La decisión de [NAME] fue confirmada por el comité.");

        let p_gallo = scrub_plain("El documento fue revisado por Juan Gallo de Andrada ayer.").text;
        assert_eq!(p_gallo, "El documento fue revisado por [NAME] ayer.");
    }

    #[test]
    fn test_spanish_negative_cases() {
        let negatives = [
            "Viajaremos a Madrid la próxima semana para la conferencia.",
            "El Lunes tendremos la reunión general del equipo.",
            "Gracias a Dios todo salió muy bien en la operación.",
            "Hoy tenemos que entregar el informe financiero sin falta.",
            "Cuando llegue el paquete, avísame por favor.",
            "Pero nadie sabía exactamente qué había ocurrido ayer.",
            "Aunque llueva mañana, iremos a trabajar temprano.",
            "Mientras tanto, el comité revisará todas las propuestas.",
            "Porque el sistema estaba en mantenimiento, no pudimos entrar.",
            "Siempre revisamos la documentación antes de publicar cambios.",
            "En Barcelona y Sevilla también abrieron nuevas oficinas.",
            "Durante el mes de Octubre se actualizarán los servidores.",
            // False-positive regressions: días, Caballero, Rey, Gallo, Curioso in common-noun context
            "Faltan veinte días para completar la entrega del proyecto.",
            "El caballero andante defendía a los menesterosos en el camino.",
            "Caballero, por favor tome asiento mientras espera su turno.",
            "El Rey convocó a las cortes para debatir las leyes.",
            "Rey de copas es una figura de la baraja española tradicional.",
            "Al cantar del gallo salieron los campesinos a cultivar la tierra.",
            "El Gallo cantó temprano en la granja del valle.",
            "Fue un suceso muy curioso y digno de mención en la crónica.",
            "El Curioso fue un personaje central de la novela intercalada.",
            // Place names: Sierra Morena and Mancha unredacted
            "El convoy atravesó Sierra Morena durante el largo viaje.",
            "El hidalgo vivía en una aldea de la Mancha.",
        ];
        for sentence in negatives {
            let out = scrub_plain(sentence).text;
            assert_eq!(out, sentence, "Spanish negative sentence was over-redacted: {}", out);
        }

        // Known over-redaction: 'Mancha' is name-leaning under the Spanish ratio rule (log10(444/477) = -0.03), so it is not gated. Do not tune the rule to fix this single case.
        let dq_out = scrub_plain("Don Quijote de la Mancha recorrió los caminos de España.").text;
        assert_eq!(dq_out, "Don [NAME] recorrió los caminos de España.");

        // Category tests for structural headings, epochs, and place names:
        // 1. All-caps document headings
        assert_eq!(scrub_plain("PRÓLOGO").text, "PRÓLOGO");
        assert_eq!(scrub_plain("NIEBLA").text, "NIEBLA");
        assert_eq!(scrub_plain("TESTIMONIO DE LAS ERRATAS").text, "TESTIMONIO DE LAS ERRATAS");
        // 2. Capitalized document sections
        assert_eq!(scrub_plain("Prólogo al lector").text, "Prólogo al lector");
        assert_eq!(scrub_plain("Dedicatoria al conde").text, "Dedicatoria al conde");
        assert_eq!(scrub_plain("Aprobaciones oficiales").text, "Aprobaciones oficiales");
        assert_eq!(scrub_plain("Tasa del libro").text, "Tasa del libro");
        assert_eq!(scrub_plain("Testimonio de fee").text, "Testimonio de fee");
        // 3. Historical epochs and institutions
        assert_eq!(scrub_plain("En la Edad Media florecieron las artes.").text, "En la Edad Media florecieron las artes.");
        assert_eq!(scrub_plain("El edificio del Colegio fue renovado.").text, "El edificio del Colegio fue renovado.");
        // 4. Common nouns with definite articles
        assert_eq!(scrub_plain("El Caballero de los Espejos").text, "El Caballero de los Espejos");
        assert_eq!(scrub_plain("Las Cortes de la Muerte").text, "Las Cortes de la Muerte");
        // 5. Spanish place names (Valladolid and Alcalá are unknown words not in dictionary, so they are redacted in Tier 3 mirroring English;
        // Verde Gabán are known dictionary words, so they remain unredacted)
        assert_eq!(scrub_plain("El viaje a Valladolid y Alcalá fue tranquilo.").text, "El viaje a [NAME] fue tranquilo.");
        assert_eq!(scrub_plain("El Caballero del Verde Gabán llegó tarde.").text, "El Caballero del Verde Gabán llegó tarde.");
    }

    #[test]
    fn test_french_pii_cases() {
        // 1. "Rene" and "René" both match the names list; output keeps the original spelling.
        let s_rene = scrub_plain("Le contrat a été signé par Rene hier.");
        assert_eq!(s_rene.text, "Le contrat a été signé par [NAME] hier.");
        assert!(s_rene.matched_name_languages.contains("fr"));

        let s_rene_accent = scrub_plain("Le contrat a été signé par René hier.");
        assert_eq!(s_rene_accent.text, "Le contrat a été signé par [NAME] hier.");
        assert!(s_rene_accent.matched_name_languages.contains("fr"));

        // 2. "l'Aubry" yields a redaction of "Aubry" only.
        let s_elision = scrub_plain("Le rapport de l'Aubry est prêt.");
        assert_eq!(s_elision.text, "Le rapport de l'[NAME] est prêt.");

        let s_elision_curly = scrub_plain("Le rapport de l’Aubry est prêt.");
        assert_eq!(s_elision_curly.text, "Le rapport de l’[NAME] est prêt.");

        // 3. "mai" (the month) is not redacted in ordinary text.
        let s_mai = scrub_plain("La réunion aura lieu en mai prochain.");
        assert_eq!(s_mai.text, "La réunion aura lieu en mai prochain.");
        assert!(!s_mai.matched_name_languages.contains("fr"));

        // 4. A string containing œ and æ runs without error; report what happens.
        let lig_input = "La sœur de Lætitia est allée à l'hôpital du Sacré-Cœur.";
        let out_lig = scrub_plain(lig_input);
        assert_eq!(out_lig.text, "La sœur de [NAME] est allée à l'hôpital du Sacré-Cœur.");

        let lig_prose = "Le fœtus et l'ex æquo ont été vérifiés.";
        let out_prose = scrub_plain(lig_prose);
        assert_eq!(out_prose.text, "Le fœtus et l'ex æquo ont été vérifiés.");
    }

    #[test]
    fn test_french_capitalization_gating() {
        // "La pierre est lourde." -> unchanged
        assert_eq!(scrub_plain("La pierre est lourde.").text, "La pierre est lourde.");

        // "Une rose rouge." -> unchanged
        assert_eq!(scrub_plain("Une rose rouge.").text, "Une rose rouge.");

        // "Il marie sa fille." -> unchanged
        assert_eq!(scrub_plain("Il marie sa fille.").text, "Il marie sa fille.");

        // "Le contrat a été signé par Pierre hier." -> Pierre redacted
        assert_eq!(
            scrub_plain("Le contrat a été signé par Pierre hier.").text,
            "Le contrat a été signé par [NAME] hier."
        );

        // "Il est parti avec PIERRE." -> PIERRE redacted
        assert_eq!(
            scrub_plain("Il est parti avec PIERRE.").text,
            "Il est parti avec [NAME]."
        );

        // "Pierre est venu hier." -> Pierre redacted (sentence start)
        assert_eq!(
            scrub_plain("Pierre est venu hier.").text,
            "[NAME] est venu hier."
        );

        // "Le rapport a été préparé par Rose hier." -> Rose redacted
        assert_eq!(
            scrub_plain("Le rapport a été préparé par Rose hier.").text,
            "Le rapport a été préparé par [NAME] hier."
        );
    }

    #[test]
    fn test_english_capitalization_gating() {
        // "Meet Rose at the park." -> Rose redacted
        assert_eq!(
            scrub_plain("Meet Rose at the park.").text,
            "Meet [NAME] at the park."
        );

        // "The rose garden was lovely." -> unchanged
        assert_eq!(
            scrub_plain("The rose garden was lovely.").text,
            "The rose garden was lovely."
        );
    }

    #[test]
    fn test_spanish_and_japanese_regression() {
        // Spanish sentences containing ambiguous words remain unchanged
        assert_eq!(scrub_plain("la rosa roja").text, "la rosa roja");
        assert_eq!(scrub_plain("el mar azul").text, "el mar azul");

        // Japanese sentence containing ordinary words remain unchanged
        let ja = "私たちは 言論の自由 を守る必要があります。";
        assert_eq!(scrub_plain(ja).text, ja);
    }

    #[test]
    fn test_portuguese_particles() {
        let p1 = scrub_plain("João da Silva foi ao mercado central.");
        assert_eq!(p1.text, "[NAME] foi ao mercado central.");
        assert!(p1.matched_name_languages.contains("pt"));

        let p2 = scrub_plain("Maria de Lurdes assinou a documentação ontem.");
        assert_eq!(p2.text, "[NAME] assinou a documentação ontem.");
        assert!(p2.matched_name_languages.contains("pt"));

        let p3 = scrub_plain("Pedro e Sousa apresentou a proposta de investimento.");
        assert_eq!(p3.text, "[NAME] apresentou a proposta de investimento.");
        assert!(p3.matched_name_languages.contains("pt"));

        let p4 = scrub_plain("António dos Santos participou da reunião executiva.");
        assert_eq!(p4.text, "[NAME] participou da reunião executiva.");
        assert!(p4.matched_name_languages.contains("pt"));

        let p5 = scrub_plain("Ana das Dores entregou o relatório financeiro.");
        assert_eq!(p5.text, "[NAME] entregou o relatório financeiro.");
        assert!(p5.matched_name_languages.contains("pt"));

        let p6 = scrub_plain("Carlos do Carmo cantou no teatro nacional.");
        assert_eq!(p6.text, "[NAME] cantou no teatro nacional.");
        assert!(p6.matched_name_languages.contains("pt"));
    }

    #[test]
    fn test_portuguese_capitalization_gating() {
        // Capitalized Rosa mid-sentence is redacted
        let s_rosa = scrub_plain("O projeto foi desenvolvido por Rosa na semana passada.");
        assert_eq!(s_rosa.text, "O projeto foi desenvolvido por [NAME] na semana passada.");
        assert!(s_rosa.matched_name_languages.contains("pt"));

        // Lowercase rosa is kept as an ordinary word
        let s_rosa_low = scrub_plain("Ela colheu uma rosa vermelha no jardim botânico.");
        assert_eq!(s_rosa_low.text, "Ela colheu uma rosa vermelha no jardim botânico.");

        // Sentence-initial Vera is redacted via name gating
        let s_vera = scrub_plain("Vera conferiu os comprovantes fiscais no sistema.");
        assert_eq!(s_vera.text, "[NAME] conferiu os comprovantes fiscais no sistema.");
        assert!(s_vera.matched_name_languages.contains("pt"));

        // Capitalized Dias mid-sentence is redacted
        let s_dias = scrub_plain("O parecer foi redigido por Dias durante o recesso.");
        assert_eq!(s_dias.text, "O parecer foi redigido por [NAME] durante o recesso.");
        assert!(s_dias.matched_name_languages.contains("pt"));

        // Lowercase dias is kept as an ordinary word
        let s_dias_low = scrub_plain("O trabalho levou cinco dias para ser concluído com sucesso.");
        assert_eq!(s_dias_low.text, "O trabalho levou cinco dias para ser concluído com sucesso.");
    }

    #[test]
    fn test_portuguese_surnames_dictionary_words() {
        // Common surnames that are also dictionary words (Costa, Pinto, Lima, Reis, Serra, Paz, Oliveira, Carvalho)
        // 1. Surname alone with a capital letter (mid-sentence)
        assert_eq!(
            scrub_plain("O parecer técnico foi elaborado por Costa no mês passado.").text,
            "O parecer técnico foi elaborado por [NAME] no mês passado."
        );
        assert_eq!(
            scrub_plain("O relatório financeiro foi assinado por Pinto ontem.").text,
            "O relatório financeiro foi assinado por [NAME] ontem."
        );
        assert_eq!(
            scrub_plain("O documento oficial foi entregue por Lima na terça-feira.").text,
            "O documento oficial foi entregue por [NAME] na terça-feira."
        );
        assert_eq!(
            scrub_plain("A decisão final foi comunicada por Reis aos funcionários.").text,
            "A decisão final foi comunicada por [NAME] aos funcionários."
        );
        assert_eq!(
            scrub_plain("O projeto arquitetónico foi desenvolvido por Serra durante o ano.").text,
            "O projeto arquitetónico foi desenvolvido por [NAME] durante o ano."
        );
        assert_eq!(
            scrub_plain("O texto da declaração foi redigido por Paz com grande rigor.").text,
            "O texto da declaração foi redigido por [NAME] com grande rigor."
        );
        assert_eq!(
            scrub_plain("A proposta comercial foi defendida por Oliveira perante o comitê.").text,
            "A proposta comercial foi defendida por [NAME] perante o comitê."
        );
        assert_eq!(
            scrub_plain("O despacho ministerial foi aprovado por Carvalho esta manhã.").text,
            "O despacho ministerial foi aprovado por [NAME] esta manhã."
        );

        // 2. After a given name
        assert_eq!(
            scrub_plain("Manuel Costa apresentou os resultados da pesquisa ontem.").text,
            "[NAME] apresentou os resultados da pesquisa ontem."
        );
        assert_eq!(
            scrub_plain("Ana Pinto foi nomeada para o novo cargo de direção.").text,
            "[NAME] foi nomeada para o novo cargo de direção."
        );
        assert_eq!(
            scrub_plain("Carlos Lima concluiu o relatório trimestral da auditoria.").text,
            "[NAME] concluiu o relatório trimestral da auditoria."
        );
        assert_eq!(
            scrub_plain("Mariana Reis assinou o documento oficial da instituição.").text,
            "[NAME] assinou o documento oficial da instituição."
        );
        assert_eq!(
            scrub_plain("Pedro Serra participou na reunião diretiva do conselho.").text,
            "[NAME] participou na reunião diretiva do conselho."
        );
        assert_eq!(
            scrub_plain("Maria Paz enviou a mensagem oficial aos colegas de trabalho.").text,
            "[NAME] enviou a mensagem oficial aos colegas de trabalho."
        );
        assert_eq!(
            scrub_plain("João Oliveira coordenou a equipa técnica com eficiência.").text,
            "[NAME] coordenou a equipa técnica com eficiência."
        );
        assert_eq!(
            scrub_plain("Beatriz Carvalho entregou a proposta final ao comitê.").text,
            "[NAME] entregou a proposta final ao comitê."
        );

        // 3. Inside a particle bridge
        assert_eq!(
            scrub_plain("Ana da Costa assinou o contrato de prestação de serviços.").text,
            "[NAME] assinou o contrato de prestação de serviços."
        );
        assert_eq!(
            scrub_plain("Manuel de Pinto entregou as chaves do novo gabinete.").text,
            "[NAME] entregou as chaves do novo gabinete."
        );
        assert_eq!(
            scrub_plain("Carlos de Lima viajou para a reunião ontem.").text,
            "[NAME] viajou para a reunião ontem."
        );
        assert_eq!(
            scrub_plain("Mariana dos Reis foi eleita para a mesa da assembleia.").text,
            "[NAME] foi eleita para a mesa da assembleia."
        );
        assert_eq!(
            scrub_plain("Pedro da Serra apresentou a sua proposta de reforma.").text,
            "[NAME] apresentou a sua proposta de reforma."
        );
        assert_eq!(
            scrub_plain("Maria da Paz fundou a nova instituição de solidariedade.").text,
            "[NAME] fundou a nova instituição de solidariedade."
        );
        assert_eq!(
            scrub_plain("João de Oliveira chefiou a delegação oficial do congresso.").text,
            "[NAME] chefiou a delegação oficial do congresso."
        );
        assert_eq!(
            scrub_plain("Beatriz de Carvalho recebeu o prémio de excelência anual.").text,
            "[NAME] recebeu o prémio de excelência anual."
        );
    }

    #[test]
    fn test_portuguese_dictionary_exclusion() {
        // Ordinary words from hunspell dictionaries are never redacted in lowercase
        let cases = [
            "Eles caminharam ao longo da costa durante a tarde inteira.",
            "A cabana ficava no topo do monte mais alto da região.",
            "Ela comprou uma lima fresca para preparar a sobremesa.",
            "O pinto pequeno nasceu ontem no ninho da fazenda.",
            "O vale verdejante separa as duas cidades históricas.",
            "A pequena vila costeira atrai milhares de turistas no verão.",
            "A preservação da mata nativa é fundamental para a bacia.",
            "Todos desejam uma convivência baseada na paz e no respeito.",
            "Os reis medievais governavam com autoridade absoluta.",
            "A serra estava completamente coberta pela névoa da manhã.",
            "O móvel antigo foi esculpido em madeira nobre de carvalho.",
            "O azeite de qualidade é extraído do fruto da oliveira.",
            "O mês de fevereiro deste ano teve vinte e nove dias.",
            "A formatura foi celebrada com imensa alegria por todos.",
            "Um verdadeiro amigo permanece presente nos momentos difíceis.",
            "O acordo proíbe expressamente o comércio ilegal de armas.",
            "O banheiro dos funcionários fica localizado no térreo.",
            "Precisamos achar uma solução prática antes do prazo final.",
            "O professor iniciou a explicação com um exemplo simples.",
            "O esquema de segurança funcionou sem interrupções.",
            "Por favor mantenha a porta fechada durante o expediente.",
            "Ele queixava-se de forte dor de cabeça após o almoço.",
            "O veículo seguiu em frente sem parar no cruzamento.",
        ];
        for case in cases {
            assert_eq!(scrub_plain(case).text, case);
        }
    }

    #[test]
    fn test_portuguese_lazy_loading() {
        // Confirm resource resolution and loading function work properly
        let dir = resolve_resource_root().expect("resolve resource root").join("pt");
        assert!(
            dir.join("pt_BR_words.tsv").exists() && dir.join("pt_PT_words.tsv").exists(),
            "Portuguese words files must exist at {:?}", dir
        );
        let res = get_pt_resources();
        assert!(!res.words.is_empty(), "pt_words must not be empty");
        assert!(!get_gated_names("pt").is_empty(), "pt_name_gated must not be empty");
        assert!(is_pt_resources_loaded(), "PT resources must be marked loaded after get_pt_resources");
    }

    #[test]
    fn test_portuguese_resource_checksums() {
        use std::io::Read;
        let dir = resolve_resource_root().expect("resolve resource root").join("pt");

        // 1. Verify existence of Portuguese resource and licence files
        for f in [
            "LICENSE_pt_BR.txt",
            "LICENSE_pt_PT.txt",
            "NOTICE_pt.md",
            "pt_name_gated.txt",
            "pt_names.txt",
            "freq/pt_freq.tsv.zst",
            "pt_BR_words.tsv",
            "pt_PT_words.tsv",
        ] {
            assert!(dir.join(f).exists(), "Resource file {} must exist in {:?}", f, dir);
        }

        // 2. Verify freq/pt_freq.tsv.zst decompresses cleanly and yields valid word\tcount pairs
        let freq_path = dir.join("freq/pt_freq.tsv.zst");
        let raw_freq_bytes = std::fs::read(&freq_path).expect("read freq/pt_freq.tsv.zst bytes");
        assert_eq!(raw_freq_bytes.len(), 2000623, "pt_freq.tsv.zst size mismatch");
        let freq_md5 = format!("{:x}", md5::compute(&raw_freq_bytes));
        assert_eq!(freq_md5, "3176a70f1c030e859ce0e6d4428dab99", "pt_freq.tsv.zst MD5 mismatch");

        let mut decoder = zstd::Decoder::new(&raw_freq_bytes[..]).expect("create zstd decoder");
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).expect("decompress pt_freq.tsv.zst");
        assert!(!buf.is_empty(), "decompressed frequency data must not be empty");
        let text = String::from_utf8_lossy(&buf);
        let first_line = text.lines().next().unwrap_or_default();
        assert!(first_line.contains('\t'), "Frequency file should contain tab-separated pairs");
    }

    // ── Hindi (hi) Unit Tests ─────────────────────────────────────────────────

    #[test]
    fn test_hindi_resource_checksums() {
        use std::io::Read;
        let dir = resolve_default_hi_resource_dir().expect("resolve hi resource dir");

        let expected: &[(&str, usize, &str)] = &[
            ("hi_words.tsv.zst", 204979, "5307f2abe8814c70a09caca7a937e0f3"),
            ("hi_freq.tsv.zst", 2375708, "b8082d3d191ae84fae77ecf4aa880bba"),
            ("hi_names_male_clean.csv", 29567, "b1c538a00d6ec72e32db7cff0f4440ac"),
            ("hi_names_female_clean.csv", 29883, "f0b638d1e61dee8a37c4c46a2fe18bda"),
            ("hi_surnames_clean.csv", 70427, "d3e30e2d2a9c3f0ef634c84e3900b7e2"),
            ("LICENSE_hi_dictionary.txt", 20138, "b296faaab9c17d5874fd17967df54736"),
            ("LICENSE_hi_names.txt", 18657, "2ab724713fdaf49e4523c4503bfd068d"),
        ];
        for &(name, exp_len, exp_md5) in expected {
            let p = dir.join(name);
            assert!(p.exists(), "Hindi resource {} must exist at {:?}", name, dir);
            let bytes = std::fs::read(&p).expect("read Hindi resource bytes");
            assert_eq!(bytes.len(), exp_len, "{} size mismatch", name);
            let md5_hex = format!("{:x}", md5::compute(&bytes));
            assert_eq!(md5_hex, exp_md5, "{} MD5 mismatch", name);
        }
        assert!(dir.join("NOTICE_hi.md").exists(), "NOTICE_hi.md must exist");

        // Verify hi_words.tsv.zst decompresses cleanly and matches exact line/word counts
        let words_bytes = std::fs::read(dir.join("hi_words.tsv.zst")).unwrap();
        let mut w_dec = zstd::Decoder::new(&words_bytes[..]).expect("create hi_words zstd decoder");
        let mut w_buf = Vec::new();
        w_dec.read_to_end(&mut w_buf).expect("decompress hi_words.tsv.zst");
        let w_text = String::from_utf8(w_buf).expect("hi_words utf8");
        let total_lines = w_text.lines().count();
        let word_rows = w_text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "word")
            .count();
        assert_eq!(total_lines, 85_019, "decompressed hi_words total lines mismatch");
        assert_eq!(word_rows, 85_013, "decompressed hi_words unique word count mismatch");

        // Verify hi_freq.tsv.zst decompresses cleanly and starts with word\tcount
        let freq_bytes = std::fs::read(dir.join("hi_freq.tsv.zst")).unwrap();
        let mut decoder = zstd::Decoder::new(&freq_bytes[..]).expect("create zstd decoder");
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).expect("decompress hi_freq.tsv.zst");
        assert!(!buf.is_empty());
        let first_data_header = String::from_utf8_lossy(&buf)
            .lines()
            .find(|l| !l.starts_with('#') && !l.is_empty())
            .unwrap_or_default()
            .to_string();
        assert_eq!(first_data_header, "word\tcount");
    }

    #[test]
    fn test_hindi_lazy_loading() {
        let dir = resolve_default_hi_resource_dir().expect("resolve hi resource dir");
        assert!(dir.join("hi_words.tsv.zst").exists(), "hi_words.tsv.zst must exist at {:?}", dir);
        let t0 = std::time::Instant::now();
        let res = get_hi_resources();
        let first_elapsed = t0.elapsed();
        let t1 = std::time::Instant::now();
        let res2 = get_hi_resources();
        let steady_elapsed = t1.elapsed();
        assert!(is_hi_resources_loaded(), "HI_RESOURCES must be loaded after get_hi_resources");
        assert_eq!(res.words.len(), 85_013);
        assert_eq!(res.non_word_names.len(), 5_487);
        assert_eq!(res.ambiguous_word_leaning.len(), 198);
        assert_eq!(res.ambiguous_name_leaning.len(), 944);
        assert_eq!(res.anchor_count, 1_618);
        assert_eq!(res.words.len(), res2.words.len());
        eprintln!(
            "[hi_lazy_load] first_access={:?} steady_state={:?} words={} non_word={} word_leaning={} name_leaning={} A={}",
            first_elapsed,
            steady_elapsed,
            res.words.len(),
            res.non_word_names.len(),
            res.ambiguous_word_leaning.len(),
            res.ambiguous_name_leaning.len(),
            res.anchor_count
        );
    }

    #[test]
    fn test_hindi_count_bound_default() {
        // T > A -> WordLeaning
        assert_eq!(classify_hi_count_bound(0, 1), HiAmbigDefault::WordLeaning);
        assert_eq!(classify_hi_count_bound(5, 6), HiAmbigDefault::WordLeaning);
        assert_eq!(classify_hi_count_bound(100, 101), HiAmbigDefault::WordLeaning);
        assert_eq!(classify_hi_count_bound(1005, 1006), HiAmbigDefault::WordLeaning);
        // T <= A -> NameLeaning (including exact boundary T == A)
        assert_eq!(classify_hi_count_bound(5, 5), HiAmbigDefault::NameLeaning);
        assert_eq!(classify_hi_count_bound(6, 5), HiAmbigDefault::NameLeaning);
        assert_eq!(classify_hi_count_bound(1005, 1005), HiAmbigDefault::NameLeaning);
        assert_eq!(classify_hi_count_bound(100, 50), HiAmbigDefault::NameLeaning);
        // T == 0 -> NameLeaning (even when A == 0)
        assert_eq!(classify_hi_count_bound(0, 0), HiAmbigDefault::NameLeaning);
        assert_eq!(classify_hi_count_bound(50, 0), HiAmbigDefault::NameLeaning);
    }

    #[test]
    fn test_hindi_runtime_rule_word_leaning_and_cues() {
        // Standalone word-leaning ambiguous names (T > A = 1005) without cue: unredacted
        assert_eq!(scrub_plain("सूर्य").text, "सूर्य");
        assert_eq!(
            scrub_plain("आज सूर्य बहुत तेज चमक रहा है।").text,
            "आज सूर्य बहुत तेज चमक रहा है।"
        );
        assert_eq!(scrub_plain("विकास").text, "विकास");
        assert_eq!(scrub_plain("सुंदर").text, "सुंदर");

        // Under Wiktionary-only dictionary + final T > A rule (A = 1618),
        // अमर (T = 1742 > 1618) is word-leaning (unredacted standalone, redacted with cue),
        // while धैर्य (T = 759 <= 1618) remains name-leaning.
        assert_eq!(scrub_plain("अमर").text, "अमर");
        assert_eq!(scrub_plain("अमर अपने घर गया।").text, "अमर अपने घर गया।");
        assert_eq!(scrub_plain("श्री अमर").text, "श्री [NAME]");
        assert_eq!(scrub_plain("नाम: अमर").text, "नाम: [NAME]");
        assert_eq!(
            scrub_plain("अमर ने कार्यालय की सभी तैयारियों की जांच की।").text,
            "[NAME] ने कार्यालय की सभी तैयारियों की जांच की।"
        );
        assert_eq!(
            scrub_plain("अमरने यह काम किया।").text,
            "[NAME]ने यह काम किया।"
        );
        assert_eq!(scrub_plain("धैर्य").text, "[NAME]");

        // Cues (title, salutation, label, or adjacent name) redact both word-leaning and name tokens
        assert_eq!(scrub_plain("श्री रोहन").text, "श्री [NAME]");
        assert_eq!(scrub_plain("श्रीमती अद्विका").text, "श्रीमती [NAME]");
        assert_eq!(scrub_plain("सुश्री अद्विका").text, "सुश्री [NAME]");
        assert_eq!(scrub_plain("डॉ. रोहन").text, "डॉ. [NAME]");
        assert_eq!(scrub_plain("प्रिय रोहन,").text, "प्रिय [NAME],");
        assert_eq!(scrub_plain("नाम: रोहन").text, "नाम: [NAME]");
        assert_eq!(scrub_plain("नाम: विकास").text, "नाम: [NAME]");
        assert_eq!(
            scrub_plain("कल सिद्धार्थ देशमुख ने नई कार्ययोजना प्रस्तुत की थी।").text,
            "कल [NAME] ने नई कार्ययोजना प्रस्तुत की थी।"
        );

        // Non-word name: always redacted even standalone
        assert_eq!(scrub_plain("अद्विका ने दस्तावेज़ भेजा।").text, "[NAME] ने दस्तावेज़ भेजा।");

        // High-frequency common-word collisions (भारत removed as place, कर/दिया removed as verb)
        assert_eq!(
            scrub_plain("भारत सरकार ने नया नियम लागू कर दिया।").text,
            "भारत सरकार ने नया नियम लागू कर दिया।"
        );
    }

    #[test]
    fn test_hindi_label_matching_and_false_positive() {
        // Explicit colon, line break, and value cases for नाम का
        assert_eq!(scrub_plain("नाम का: सिद्धार्थ देशमुख").text, "नाम का: [NAME]");
        assert_eq!(scrub_plain("नाम का\nसिद्धार्थ देशमुख").text, "नाम का\n[NAME]");
        assert_eq!(scrub_plain("नाम का सिद्धार्थ देशमुख").text, "नाम का [NAME]");
        // Label false-positive guard: नाम followed by common word does not capture has/verb;
        // राम is Wiktionary pos=="name" only (non-word under pos != "name"), while शांति is word-leaning.
        assert_eq!(scrub_plain("शांति का नाम है").text, "शांति का नाम है");
        assert_eq!(scrub_plain("इस नाम का अर्थ क्या है।").text, "इस नाम का अर्थ क्या है।");
        assert_eq!(scrub_plain("राम का नाम है").text, "[NAME] का नाम है");
    }

    #[test]
    fn test_hindi_namelist_cleanup_drops_function_words() {
        let dir = resolve_default_hi_resource_dir().expect("resolve hi resource dir");
        let mut clean_all = HashSet::new();
        load_hi_csv_names(&dir.join("hi_names_male_clean.csv"), &mut clean_all);
        load_hi_csv_names(&dir.join("hi_names_female_clean.csv"), &mut clean_all);
        load_hi_csv_names(&dir.join("hi_surnames_clean.csv"), &mut clean_all);

        // All-function-tag entries dropped
        assert!(!clean_all.contains("हो"));
        assert!(!clean_all.contains("या"));
        assert!(!clean_all.contains("बिना"));
        assert!(!clean_all.contains("भर"));
        assert!(!clean_all.contains("जाना"));

        // Any entry carrying Verb (or other non-nominal tag) dropped even with noun/name
        assert!(!clean_all.contains("कर"));
        assert!(!clean_all.contains("दे"));
        assert!(!clean_all.contains("दिया"));

        // Review list restored except वाले; adverb-without-name rule drops केवल, नित्य, माने, सदा.
        assert!(!clean_all.contains("वाले"));
        assert!(clean_all.contains("कोहली"));
        assert!(clean_all.contains("रोहित"));
        assert!(!clean_all.contains("केवल"));
        assert!(!clean_all.contains("नित्य"));
        assert!(!clean_all.contains("माने"));
        assert!(!clean_all.contains("सदा"));
        assert!(clean_all.contains("आदि"));
        assert!(clean_all.contains("निश्चय"));

        // Country/state/city/place-name entries removed (e.g. भारत)
        assert!(!clean_all.contains("भारत"));

        let res = get_hi_resources();
        assert!(!res.ambiguous_word_leaning.contains("कर"));
        assert!(!res.ambiguous_word_leaning.contains("दिया"));
        assert!(!res.ambiguous_word_leaning.contains("केवल"));
        assert!(!res.non_word_names.contains("भारत"));
        assert!(res.non_word_names.contains("कोहली"));
        assert!(res.non_word_names.contains("रोहित"));
    }

    #[test]
    fn test_hindi_devanagari_normalization() {
        // Precomposed U+0958..U+095F canonicalize to base + U+093C (same as NFC)
        assert_eq!(normalize_hi_str("\u{095C}\u{093E}"), normalize_hi_str("ड\u{093C}ा")); // ड़ा vs ड + ़ + ा
        // ऩ/ऱ/ऴ decompose consistently to base + U+093C
        assert_eq!(normalize_hi_str("क\u{0931}्हाडे"), normalize_hi_str("कर\u{093C}्हाडे"));
        assert_eq!(normalize_hi_str("\u{0929}"), normalize_hi_str("न\u{093C}"));
        // Post-vowel nukta reordering (पीडि़त vs पीड़ित, बडे़ vs बड़े)
        assert_eq!(normalize_hi_str("पीडि\u{093C}त"), normalize_hi_str("पीड़ित"));
        assert_eq!(normalize_hi_str("बडे\u{093C}"), normalize_hi_str("बड़े"));
        // Anusvara (ं U+0902) and chandrabindu (ँ U+0901) remain distinct
        assert_ne!(normalize_hi_str("हंस"), normalize_hi_str("हँस"));
        assert_ne!(normalize_hi_str("अंजु"), normalize_hi_str("अँजु"));
    }

    #[test]
    fn test_korean_section_b_positive_and_negative_cases() {
        // 10+ Positive Korean cases:
        let positives = [
            // 1. Unspaced adjacent surname + given name
            ("오늘 회의에는 김민수 연구원이 참석합니다.", "오늘 회의에는 [NAME] 연구원이 참석합니다."),
            // 2. Spaced 2-syllable surname + given name
            ("이번 과제는 남궁 민준 연구원이 담당합니다.", "이번 과제는 [NAME] 연구원이 담당합니다."),
            // 3. Particle-attached name (는, 은, 이, 가, 을, 를, 에게)
            ("박지훈은 새로운 기획안을 발표했습니다.", "[NAME]은 새로운 기획안을 발표했습니다."),
            ("이서연이 회의록을 작성하여 공유했습니다.", "[NAME]이 회의록을 작성하여 공유했습니다."),
            ("최우진을 프로젝트 책임자로 지정했습니다.", "[NAME]을 프로젝트 책임자로 지정했습니다."),
            ("정하윤에게 최종 보고서를 전달했습니다.", "[NAME]에게 최종 보고서를 전달했습니다."),
            // 4. Honorifics after a name (씨, 님, 선생님, 과장님, 부장님, 대표님, 교수님) — honorific preserved
            ("김민수 씨가 오후에 방문할 예정입니다.", "[NAME] 씨가 오후에 방문할 예정입니다."),
            ("박지훈 님께서 서류를 제출하셨습니다.", "[NAME] 님께서 서류를 제출하셨습니다."),
            ("최서연 선생님께 문의 메일을 보냈습니다.", "[NAME] 선생님께 문의 메일을 보냈습니다."),
            ("정민호 과장님이 예산을 승인했습니다.", "[NAME] 과장님이 예산을 승인했습니다."),
            ("강동현 부장님과 일정을 조율했습니다.", "[NAME] 부장님과 일정을 조율했습니다."),
            ("윤서준 대표님이 신년사를 발표했습니다.", "[NAME] 대표님이 신년사를 발표했습니다."),
            ("한지민 교수님의 강의를 수강했습니다.", "[NAME] 교수님의 강의를 수강했습니다."),
            // 5. Titles before a name (과장 김민수, 부장님 박지훈) — title preserved
            ("과장 김민수에게 결재를 요청했습니다.", "과장 [NAME]에게 결재를 요청했습니다."),
            ("부장님 박지훈이 회의를 주재했습니다.", "부장님 [NAME]이 회의를 주재했습니다."),
            // 6. Labeled fields (이름:, 성명:, 성:, 명:)
            ("이름: 김민수", "이름: [NAME]"),
            ("성명: 박지훈", "성명: [NAME]"),
            ("성: 최", "성: [NAME]"),
            ("명: 서연", "명: [NAME]"),
        ];
        for (input, expected) in positives {
            let out = scrub_plain(input).text;
            assert_eq!(out, expected, "Korean positive case failed for input: {}", input);
        }

        // 10+ Negative Korean cases (ordinary Hangul sentences with 이, 박, 최, 정 as particles/common words,
        // 2-syllable common words, and lone given names without cues):
        let negatives = [
            "이 회사는 올해 매출 목표를 달성했습니다.",
            "박물관에서 열린 특별 전시회를 관람했습니다.",
            "최선을 다해 이번 프로젝트를 마무리합시다.",
            "정확한 일정을 확인한 후에 다시 연락드리겠습니다.",
            "이 보고서는 내일 오전 회의에서 논의할 예정입니다.",
            "정부는 새로운 경제 지원 정책을 발표했습니다.",
            "최근 기술 동향에 대한 자료를 수집하고 있습니다.",
            "박수를 치며 모든 참석자가 결과를 축하했습니다.",
            "오늘 오후에 팀원들과 함께 점심을 먹었습니다.",
            "회의실 예약 현황을 사내 시스템에서 확인하세요.",
            "하늘에 구름이 많고 바람이 시원하게 붑니다.",
        ];
        for sentence in negatives {
            let out = scrub_plain(sentence).text;
            assert_eq!(out, sentence, "Korean negative sentence was over-redacted: {}", out);
        }
    }

    // ── Bangla (bn) Unit Tests ────────────────────────────────────────────────

    #[test]
    fn test_bn_known_word_never_redacted() {
        // Words in bn_words.tsv must never be redacted alone or next to another known word
        assert!(is_bn_known_word("আজকের"));
        assert!(is_bn_known_word("প্রযুক্তিগত"));
        assert!(is_bn_known_word("সভার"));
        assert!(is_bn_known_word("আয়োজন"));
        assert!(is_bn_known_word("করেছে"));
        let sentence = "আজকের প্রযুক্তিগত সভার আয়োজন করেছে।";
        let rep = scrub_plain(sentence);
        assert_eq!(
            rep.text, sentence,
            "Known Bangla words must never be redacted by the Bangla tier: {}",
            rep.text
        );
    }

    #[test]
    fn test_bn_word_name_overlap_treated_as_name_and_word_set_flag_invariant() {
        // A token present in both the word list and the names list (e.g. "বিজয়", "রায়", "অনিক", "রবি", "লেখা")
        // is subtracted from the word set and treated as a name.
        assert!(!is_bn_known_word("বিজয়"));
        assert!(is_bn_name("বিজয়"));
        assert!(!is_bn_known_word("রায়"));
        assert!(is_bn_name("রায়"));
        assert!(!is_bn_known_word("অনিক"));
        assert!(is_bn_name("অনিক"));
        assert!(!is_bn_known_word("রবি"));
        assert!(is_bn_name("রবি"));
        assert_eq!(classify_bn_token("রবি"), BnTokKind::Name);
        assert!(!is_bn_known_word("লেখা"));
        assert!(is_bn_name("লেখা"));
        assert_eq!(classify_bn_token("লেখা"), BnTokKind::Name);

        let out_bijoy = scrub_plain("বিজয় আজকের প্রযুক্তিগত সভার আয়োজন করেছে।").text;
        assert_eq!(out_bijoy, "[NAME] আজকের প্রযুক্তিগত সভার আয়োজন করেছে।");

        let out_lekha = scrub_plain("গতকাল লেখা একটি নতুন বই কিনেছেন।").text;
        assert_eq!(out_lekha, "গতকাল [NAME] একটি নতুন বই কিনেছেন।");

        // Also verify runtime subtraction on a custom directory where a word appears in both files
        // across different source labels (wikipedia_category, gemini_unverified, manual_verification).
        let tmp_dir = std::env::temp_dir().join("philosofile_bn_subtraction_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        std::fs::write(
            tmp_dir.join("bn_words.tsv"),
            "# header\nword\tsources\nবই\tbnwiktionary_titles\nবিজয়\thermitdave_bn50k\nতন্ময়\thermitdave_bn50k\n",
        )
        .unwrap();
        std::fs::write(
            tmp_dir.join("bn_names.tsv"),
            "# header\nname\tromanized\tgender\tsource\nবিজয় সরকার\t\t\twikipedia_category\nতন্ময়\tTonmoy\tM\tgemini_unverified\n",
        )
        .unwrap();

        let res = load_bn_resources_from_dir(&tmp_dir);
        assert!(res.words.contains(&normalize_bn_str("বই")));
        assert!(
            !res.words.contains(&normalize_bn_str("বিজয়")),
            "Name token from wikipedia_category must be subtracted from word set at runtime"
        );
        assert!(
            !res.words.contains(&normalize_bn_str("তন্ময়")),
            "Name token from gemini_unverified must also be subtracted from word set at runtime"
        );
        assert!(res.names.contains(&normalize_bn_str("বিজয়")));
        assert!(res.names.contains(&normalize_bn_str("তন্ময়")));

        let _ = std::fs::remove_dir_all(&tmp_dir);

        // "অনিক" is handled via bn_names.tsv rather than a hardcoded special case.
        let rep_anik = scrub_plain("গতকাল অনিক একটি নতুন বই কিনেছেন।");
        assert_eq!(rep_anik.text, "গতকাল [NAME] একটি নতুন বই কিনেছেন।");
    }

    #[test]
    fn test_bn_known_word_between_two_names_not_redacted() {
        // A known word between two names is not redacted, while the names are.
        assert!(is_bn_name("অনিক"));
        assert!(is_bn_name("ইউনূস"));
        assert!(is_bn_known_word("এবং"));
        assert!(is_bn_known_word("বই"));

        let s1 = "গতকাল অনিক এবং ইউনূস উপস্থিত ছিলেন।";
        let out1 = scrub_plain(s1);
        assert_eq!(
            out1.text,
            "গতকাল [NAME] এবং [NAME] উপস্থিত ছিলেন।",
            "Known word 'এবং' between two names must not be redacted: {}",
            out1.text
        );
        assert_eq!(out1.counts.get("NAME"), Some(&2));

        // Known word alone or next to another known word is not redacted
        let s2 = "বই";
        assert_eq!(scrub_plain(s2).text, s2);
        let s3 = "নতুন বই";
        assert_eq!(scrub_plain(s3).text, s3);
    }

    #[test]
    fn test_bn_name_list_token_redacted() {
        // "মজুমদার" and "ইউনূস" are in bn_names.tsv (wikipedia_category) and not in bn_words.tsv
        assert!(is_bn_name("মজুমদার"));
        assert!(!is_bn_known_word("মজুমদার"));
        assert!(is_bn_name("ইউনূস"));
        assert!(!is_bn_known_word("ইউনূস"));

        let input = "গতকাল ইউনূস একটি নতুন বই কিনেছেন।";
        let rep = scrub_plain(input);
        assert_eq!(rep.text, "গতকাল [NAME] একটি নতুন বই কিনেছেন।");
        assert!(rep.matched_name_languages.contains("bn"));
        assert!(rep.unreviewed_languages.contains("bn"));
    }

    #[test]
    fn test_bn_name_token_next_to_unknown_token_redacts_both() {
        // "মজুমদার" is a name-list token; "অমিতাংশু" is in neither bn_words.tsv nor bn_names.tsv
        assert!(is_bn_name("মজুমদার"));
        assert!(!is_bn_known_word("অমিতাংশু"));
        assert!(!is_bn_name("অমিতাংশু"));

        let input = "গতকাল অমিতাংশু মজুমদার সভায় উপস্থিত ছিলেন।";
        let rep = scrub_plain(input);
        assert_eq!(
            rep.text,
            "গতকাল [NAME] সভায় উপস্থিত ছিলেন।",
            "Expected both unknown token and adjacent name-list token to be redacted as one span, got: {}",
            rep.text
        );
        assert!(!rep.text.contains("অমিতাংশু"));
        assert!(!rep.text.contains("মজুমদার"));
    }

    #[test]
    fn test_bn_full_name_2_and_3_tokens_redacted_as_one_unit() {
        // 2-token full name from wikipedia_category
        let input_2 = "গতকাল মুহাম্মদ ইউনূস বক্তব্য দিয়েছেন।";
        let rep_2 = scrub_plain(input_2);
        assert_eq!(rep_2.text, "গতকাল [NAME] বক্তব্য দিয়েছেন।");
        assert_eq!(rep_2.counts.get("NAME"), Some(&1));

        // 3-token full name "মোহাম্মদ রফিক আহমেদ" caught as one span
        let input_3 = "গতকাল মোহাম্মদ রফিক আহমেদ এই প্রকল্পের অগ্রগতি পর্যালোচনা করেছেন।";
        let rep_3 = scrub_plain(input_3);
        assert_eq!(
            rep_3.text,
            "গতকাল [NAME] এই প্রকল্পের অগ্রগতি পর্যালোচনা করেছেন।"
        );
        assert_eq!(rep_3.counts.get("NAME"), Some(&1));

        // 3-token full name "চন্দন রায় সান্যাল" where all 3 parts are Name tokens after word/name subtraction
        assert!(!is_bn_known_word("রায়"));
        assert!(is_bn_name("রায়"));
        let input_3_word = "গতকাল চন্দন রায় সান্যাল উপস্থিত ছিলেন।";
        let rep_3_word = scrub_plain(input_3_word);
        assert_eq!(rep_3_word.text, "গতকাল [NAME] উপস্থিত ছিলেন।");
        assert_eq!(rep_3_word.counts.get("NAME"), Some(&1));
    }

    #[test]
    fn test_bn_unknown_token_without_name_neighbor_not_redacted_unless_cued() {
        // "অমিতাংশু" is in neither bn_words.tsv nor bn_names.tsv
        assert!(!is_bn_known_word("অমিতাংশু"));
        assert!(!is_bn_name("অমিতাংশু"));

        // Standalone with no name-list neighbor and no cue -> NOT redacted by Bangla tier
        let bare = "গতকাল অমিতাংশু একটি নতুন বই কিনেছেন।";
        let rep_bare = scrub_plain(bare);
        assert_eq!(
            rep_bare.text, bare,
            "Unknown Bangla token with no name-list neighbor and no cue must not be redacted: {}",
            rep_bare.text
        );

        // With honorific cues (জনাব, মিসেস) or label (নাম:) -> redacted!
        assert_eq!(
            scrub_plain("জনাব অমিতাংশু একটি নতুন বই কিনেছেন।").text,
            "জনাব [NAME] একটি নতুন বই কিনেছেন।"
        );
        assert_eq!(
            scrub_plain("মিসেস তাসনিম সভায় উপস্থিত ছিলেন।").text,
            "মিসেস [NAME] সভায় উপস্থিত ছিলেন।"
        );
        assert_eq!(scrub_plain("নাম: অমিতাংশু\n").text, "নাম: [NAME]\n");
    }

    #[test]
    fn test_bn_missing_resource_file_returns_empty_and_never_panics() {
        let missing_dir = std::env::temp_dir().join("philosofile_nonexistent_bn_resources_99999");

        // Debug mode path: logs warning, load_errors is empty, returns empty sets without panicking
        let res_debug = load_bn_resources_from_dir_with_mode(&missing_dir, false);
        assert!(res_debug.words.is_empty());
        assert!(res_debug.names.is_empty());
        assert!(res_debug.full_names.is_empty());
        assert!(
            res_debug.load_errors.is_empty(),
            "Debug mode should warn without recording release load_errors"
        );
        assert!(res_debug.words_error.is_none());

        let mut opts_debug = ScrubOptions::without_identity();
        opts_debug.bn_resource_dir = Some(missing_dir.clone());
        opts_debug.bn_release_mode_override = Some(false);
        let input = "গতকাল মুহাম্মদ ইউনূস একটি নতুন বই কিনেছেন।";
        let rep_debug = scrub_text_with(input, &opts_debug);
        assert!(rep_debug.error.is_none());
        assert_eq!(rep_debug.text, input);
        assert!(try_scrub_text_with(input, &opts_debug).is_ok());

        // Release mode path (missing bn_words.tsv): scrubbing refuses and returns a clear error
        let res_release = load_bn_resources_from_dir_with_mode(&missing_dir, true);
        assert!(res_release.words.is_empty());
        assert!(res_release.words_error.is_some());
        assert_eq!(
            res_release.load_errors.len(),
            3,
            "Release mode must record an error for missing bn_words.tsv, bn_names.tsv, and bn_names_gemini.tsv"
        );

        let mut opts_release_missing = ScrubOptions::without_identity();
        opts_release_missing.bn_resource_dir = Some(missing_dir);
        opts_release_missing.bn_release_mode_override = Some(true);
        let rep_release_missing = scrub_text_with(input, &opts_release_missing);
        assert!(
            rep_release_missing.error.is_some(),
            "Release build must refuse scrubbing when bn_words.tsv is missing"
        );
        assert!(rep_release_missing.text.is_empty());
        let try_err = try_scrub_text_with(input, &opts_release_missing);
        assert!(try_err.is_err());
        assert!(try_err.unwrap_err().contains("ERROR:"));

        // Release mode path (empty bn_words.tsv): scrubbing also refuses and returns a clear error
        let empty_dir = std::env::temp_dir().join("philosofile_empty_bn_words_99999");
        let _ = std::fs::create_dir_all(&empty_dir);
        std::fs::write(empty_dir.join("bn_words.tsv"), "# header only\nword\tsources\n").unwrap();
        std::fs::write(
            empty_dir.join("bn_names.tsv"),
            "# header\nname\tromanized\tgender\tsource\nরবি\tRabi\tM\tmanual_verification\n",
        )
        .unwrap();

        let mut opts_release_empty = ScrubOptions::without_identity();
        opts_release_empty.bn_resource_dir = Some(empty_dir.clone());
        opts_release_empty.bn_release_mode_override = Some(true);
        let rep_release_empty = scrub_text_with(input, &opts_release_empty);
        assert!(
            rep_release_empty.error.is_some(),
            "Release build must refuse scrubbing when bn_words.tsv is empty"
        );
        assert!(rep_release_empty.text.is_empty());
        assert!(try_scrub_text_with(input, &opts_release_empty).is_err());
        let _ = std::fs::remove_dir_all(&empty_dir);
    }

    #[test]
    fn test_bn_suffix_stripping_layer() {
        let bn_res = get_bn_resources();
        // Confirm no suffix in bn_suffixes.tsv is shorter than 2 characters
        for suf in bn_res.suffixes.iter() {
            assert!(
                suf.chars().count() >= 2,
                "Every suffix in bn_suffixes.tsv must have >= 2 characters, found {:?} (len={})",
                suf,
                suf.chars().count()
            );
        }

        // 1. A name followed by an inflected word is still redacted as a name (and the inflected word is NOT redacted).
        // "রবি" is in bn_names.tsv; "রিক্সায়" is not an exact match in bn_words.tsv,
        // but stripping suffix "য়" yields stem "রিক্সা" which is in bn_words.tsv -> classed as Word.
        assert_eq!(classify_bn_token("রবি"), BnTokKind::Name);
        assert!(!bn_res.words.contains(&normalize_bn_str("রিক্সায়")));
        assert!(bn_res.words.contains(&normalize_bn_str("রিক্সা")));
        assert_eq!(classify_bn_token("রিক্সায়"), BnTokKind::Word);

        let s_name_inflected = "গতকাল রবি রিক্সায় এসেছেন।";
        let rep1 = scrub_plain(s_name_inflected);
        assert_eq!(
            rep1.text,
            "গতকাল [NAME] রিক্সায় এসেছেন।",
            "Name followed by inflected word must redact only the name: {}",
            rep1.text
        );

        // Also test with an unknown token before/after an inflected word so the inflected word is not swept in:
        // "ক্লাসগুলোকে" -> strip "গুলোকে" -> stem "ক্লাস" in bn_words.tsv -> Word
        assert!(!bn_res.words.contains(&normalize_bn_str("ক্লাসগুলোকে")));
        assert_eq!(classify_bn_token("ক্লাসগুলোকে"), BnTokKind::Word);
        let s_name_inflected_2 = "গতকাল অনিক ক্লাসগুলোকে পরিদর্শন করেছেন।";
        assert_eq!(
            scrub_plain(s_name_inflected_2).text,
            "গতকাল [NAME] ক্লাসগুলোকে পরিদর্শন করেছেন।"
        );

        // 2. An inflected dictionary verb is classed as Word and not redacted.
        // "আঁকছিলেন" -> strip "ছিলেন" -> stem "আঁক" in bn_words.tsv -> classed as Word.
        assert!(!bn_res.words.contains(&normalize_bn_str("আঁকছিলেন")));
        assert!(bn_res.words.contains(&normalize_bn_str("আঁক")));
        assert_eq!(classify_bn_token("আঁকছিলেন"), BnTokKind::Word);
        let s_verb = "গতকাল অনিক আঁকছিলেন।";
        assert_eq!(
            scrub_plain(s_verb).text,
            "গতকাল [NAME] আঁকছিলেন।",
            "Inflected dictionary verb 'আঁকছিলেন' must be classed as Word and not redacted"
        );

        // 3. A name that happens to end in a suffix string is still matched as a name (exact name check runs before stripping).
        // Even if the stem after stripping a suffix were in bn_words.tsv,
        // exact match in bn_names.tsv always wins first.
        assert_eq!(classify_bn_token("সরকার"), BnTokKind::Name);
        assert_eq!(classify_bn_token("তালুকদার"), BnTokKind::Name);
        assert_eq!(classify_bn_token("জহির"), BnTokKind::Name);
        let s_suffix_name = "গতকাল সরকার এবং তালুকদার উপস্থিত ছিলেন।";
        assert_eq!(
            scrub_plain(s_suffix_name).text,
            "গতকাল [NAME] এবং [NAME] উপস্থিত ছিলেন।"
        );

        // Also verify in a custom resource dir where a name "বইরা" is in bn_names.tsv and its stem "বই" is in bn_words.tsv:
        // the exact name check must classify "বইরা" as Name, not Word.
        let tmp_dir = std::env::temp_dir().join("philosofile_bn_suffix_order_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        std::fs::write(
            tmp_dir.join("bn_words.tsv"),
            "# header\nword\tsources\nবই\tbnwiktionary_titles\n",
        )
        .unwrap();
        std::fs::write(
            tmp_dir.join("bn_names.tsv"),
            "# header\nname\tromanized\tgender\tsource\nবইরা\tBoira\tF\tmanual_verification\n",
        )
        .unwrap();
        let res_custom = load_bn_resources_from_dir(&tmp_dir);
        assert_eq!(
            classify_bn_token_in("বইরা", &res_custom),
            BnTokKind::Name,
            "A name that ends in a suffix whose stem is in bn_words.tsv must still be classified as Name"
        );
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_bn_all_name_rows_loaded_regardless_of_source() {
        // Every row in bn_names.tsv and bn_names_gemini.tsv is active by default, regardless of its source label
        // (manual_verification, wikipedia_category, gemini_unverified, or any other label).
        let bn_res = get_bn_resources();
        let bn_dir = resolve_default_bn_resource_dir().expect("resolve bn resource dir");

        let mut file_counts = Vec::new();
        let mut total_rows = 0usize;
        for file_name in ["bn_names.tsv", "bn_names_gemini.tsv"] {
            let names_path = bn_dir.join(file_name);
            let content = std::fs::read_to_string(&names_path)
                .unwrap_or_else(|e| panic!("{} should be readable: {}", file_name, e));
            let mut file_rows = 0usize;
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let cols: Vec<&str> = line.split('\t').collect();
                let name_raw = cols[0].trim();
                if idx <= 5 && name_raw == "name" {
                    continue;
                }
                let norm_name = normalize_bn_str(name_raw);
                if !norm_name.is_empty() {
                    file_rows += 1;
                    assert!(
                        bn_res.full_names.contains(&norm_name),
                        "Row {} ({:?}) from {} must be loaded into full_names",
                        idx + 1,
                        name_raw,
                        file_name
                    );
                    assert!(
                        is_bn_name(&norm_name),
                        "Row {} ({:?}) from {} must be active in is_bn_name",
                        idx + 1,
                        name_raw,
                        file_name
                    );
                    assert!(
                        !bn_res.words.contains(&norm_name),
                        "Row {} ({:?}) from {} must have zero overlap with bn_words.tsv",
                        idx + 1,
                        name_raw,
                        file_name
                    );
                    for part in norm_name.split_whitespace() {
                        if part.chars().count() >= 2 {
                            assert!(
                                is_bn_name(part),
                                "Token {:?} from {} row {} must be active in is_bn_name",
                                part,
                                file_name,
                                idx + 1
                            );
                            assert!(
                                !bn_res.words.contains(part),
                                "Token {:?} from {} row {} must be absent from bn_words.tsv",
                                part,
                                file_name,
                                idx + 1
                            );
                        }
                    }
                }
            }
            file_counts.push((file_name, file_rows));
            total_rows += file_rows;
        }
        assert_eq!(file_counts[0], ("bn_names.tsv", 180));
        assert_eq!(file_counts[1], ("bn_names_gemini.tsv", 2344));
        assert_eq!(total_rows, 2524);

        // Specifically verify gemini_unverified rows like "আব্দুল্লাহ" and "লেখা" are active by default
        assert!(!is_bn_known_word("আব্দুল্লাহ"));
        assert!(is_bn_name("আব্দুল্লাহ"));
        assert_eq!(classify_bn_token("আব্দুল্লাহ"), BnTokKind::Name);
        assert!(!is_bn_known_word("লেখা"));
        assert!(is_bn_name("লেখা"));
        assert_eq!(classify_bn_token("লেখা"), BnTokKind::Name);

        let input = "গতকাল আব্দুল্লাহ একটি নতুন বই কিনেছেন।";
        let rep = scrub_plain(input);
        assert_eq!(rep.text, "গতকাল [NAME] একটি নতুন বই কিনেছেন।");
        assert!(rep.matched_name_languages.contains("bn"));

        let romans = bn_name_romanizations("আব্দুল্লাহ");
        assert!(
            romans.contains(&"Abdullah".to_string()),
            "Expected romanizations for আব্দুল্লাহ, got: {:?}",
            romans
        );
    }

    #[test]
    fn test_bn_comment_lines_in_tsv_skipped() {
        // Comment lines starting with '#' in bn_words.tsv and bn_names.tsv must never be loaded as entries
        assert!(!is_bn_known_word("#"));
        assert!(!is_bn_name("#"));
        assert!(!is_bn_known_word("word"));
        assert!(!is_bn_name("name"));

        let tmp_dir = std::env::temp_dir().join("philosofile_bn_comment_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        std::fs::write(
            tmp_dir.join("bn_words.tsv"),
            "# মেঘনাদ comment line\nword\tsources\nবই\tbnwiktionary_titles\n",
        )
        .unwrap();
        std::fs::write(
            tmp_dir.join("bn_names.tsv"),
            "# শারমিন\tSharmin\tF\twikipedia_category\nname\tromanized\tgender\tsource\nচারু মজুমদার\t\t\twikipedia_category\n",
        )
        .unwrap();

        let res = load_bn_resources_from_dir(&tmp_dir);
        assert!(!res.words.contains("# মেঘনাদ comment line"));
        assert!(!res.words.contains("মেঘনাদ"));
        assert!(res.words.contains(&normalize_bn_str("বই")));
        assert!(!res.names.contains(&normalize_bn_str("# শারমিন")));
        assert!(!res.names.contains(&normalize_bn_str("শারমিন")));
        assert!(res.names.contains(&normalize_bn_str("চারু")));
        assert!(res.names.contains(&normalize_bn_str("মজুমদার")));
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_bn_fragment_of_longer_word_not_redacted() {
        // Do not redact a token that is only a fragment of a longer word.
        // "মজুমদার" and "সেন" are in the name list, but inside longer words or hyphenated compounds
        // (e.g. "মজুমদারপাড়া", "সেনাবাহিনী", or hyphenated "মজুমদার-পাড়া") they are fragments and must not be redacted.
        let s1 = "আমরা সেনাবাহিনী ও মজুমদারপাড়া এলাকায় বসবাস করেন।";
        assert_eq!(scrub_plain(s1).text, s1);

        let s2 = "তিনি মজুমদার-পাড়া সড়কে হাঁটছিলেন।";
        assert_eq!(scrub_plain(s2).text, s2);
    }

    #[test]
    fn test_ipadic_decompression_and_cache_reuse() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let test_base_dir = std::env::temp_dir().join(format!("test_ipadic_cache_{}_{}", std::process::id(), unique_id));
        if test_base_dir.exists() {
            let _ = std::fs::remove_dir_all(&test_base_dir);
        }

        // First run: decompress all nine files
        let cache_dir = ensure_ipadic_decompressed(Some(&test_base_dir))
            .expect("First run of ensure_ipadic_decompressed must succeed");
        assert!(cache_dir.is_dir());
        assert!(is_ipadic_cache_valid(&cache_dir));

        for file_name in IPADIC_FILES {
            let file_path = cache_dir.join(file_name);
            assert!(file_path.is_file(), "File {:?} must exist after decompression", file_name);
            let meta = std::fs::metadata(&file_path).expect("File metadata must be readable");
            assert!(meta.len() > 0, "File {:?} must be non-empty", file_name);
        }

        let words_meta_1 = std::fs::metadata(cache_dir.join("dict.words")).unwrap();
        let mtime_1 = words_meta_1.modified().ok();

        // Second run: reuse cache without re-decompression
        let cache_dir_2 = ensure_ipadic_decompressed(Some(&test_base_dir))
            .expect("Second run of ensure_ipadic_decompressed must succeed");
        assert_eq!(cache_dir, cache_dir_2);

        let words_meta_2 = std::fs::metadata(cache_dir.join("dict.words")).unwrap();
        let mtime_2 = words_meta_2.modified().ok();
        assert_eq!(mtime_1, mtime_2, "Cache must be reused without rewriting files");

        let _ = std::fs::remove_dir_all(&test_base_dir);
    }

    #[test]
    fn test_ipadic_unwritable_directory_disables_gracefully() {
        // Point to an invalid/unwritable directory
        let unwritable_dir = PathBuf::from("/dev/null/impossible_dir_cannot_create");
        let res = ensure_ipadic_decompressed(Some(&unwritable_dir));
        assert!(res.is_err(), "ensure_ipadic_decompressed must fail on unwritable path");

        // Verify that Japanese text scrubbing does not panic even if segmenter is disabled
        let sample_ja = "田中さんは東京に行きました。";
        let report = scrub_plain(sample_ja);
        assert!(!report.text.is_empty(), "Scrubbing must complete without panicking");
    }

    #[test]
    fn test_lazy_loading_non_japanese_scans_never_create_cache() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let test_base = std::env::temp_dir().join(format!("test_lazy_cache_{}_{}", std::process::id(), unique_id));
        let _ = std::fs::remove_dir_all(&test_base);
        std::fs::create_dir_all(&test_base).unwrap();

        init_test_ja_resource_dir();

        let cache_v1 = test_base.join(IPADIC_CACHE_DIR_NAME);
        assert!(!cache_v1.exists(), "Cache dir must not exist before any scans");

        // 1. Purely English scan
        let en_text = "Hello John, please meet Mary at the office on Monday.";
        assert!(!en_text.chars().any(is_cjk_or_japanese));
        let en_rep = scrub_plain(en_text);
        assert!(!en_rep.text.is_empty());
        assert!(!cache_v1.exists(), "English scan must never create ja cache directory");

        // 2. Purely French scan
        let fr_text = "Bonjour Pierre, le contrat a été signé par Julien hier.";
        assert!(!fr_text.chars().any(is_cjk_or_japanese));
        let fr_rep = scrub_plain(fr_text);
        assert!(!fr_rep.text.is_empty());
        assert!(!cache_v1.exists(), "French scan must never create ja cache directory");

        // 3. Spanish scan
        let es_text = "Hola Carlos, nos vemos en la biblioteca central.";
        assert!(!es_text.chars().any(is_cjk_or_japanese));
        let es_rep = scrub_plain(es_text);
        assert!(!es_rep.text.is_empty());
        assert!(!cache_v1.exists(), "Spanish scan must never create ja cache directory");

        // 4. Decompression for Japanese path creates the cache once
        let decompressed = ensure_ipadic_decompressed(Some(&test_base))
            .expect("Decompression for Japanese path must succeed");
        assert_eq!(decompressed, cache_v1);
        assert!(cache_v1.exists(), "Japanese decompression must create ja cache directory");
        assert!(is_ipadic_cache_valid(&cache_v1), "Cache must be fully populated with all valid IPADIC files");

        // 5. Subsequent call reuses the cache directory without recreation
        let words_meta_1 = std::fs::metadata(cache_v1.join("dict.words")).unwrap();
        let decompressed_2 = ensure_ipadic_decompressed(Some(&test_base))
            .expect("Subsequent call must succeed");
        assert_eq!(decompressed_2, cache_v1);
        let words_meta_2 = std::fs::metadata(cache_v1.join("dict.words")).unwrap();
        assert_eq!(words_meta_1.modified().ok(), words_meta_2.modified().ok(), "Cache must be reused once created");

        let _ = std::fs::remove_dir_all(&test_base);
    }

    #[test]
    fn test_all_paths_to_loader_succeed_and_create_cache() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let test_base = std::env::temp_dir().join(format!("test_all_paths_cache_{}_{}", std::process::id(), unique_id));
        let _ = std::fs::remove_dir_all(&test_base);
        std::fs::create_dir_all(&test_base).unwrap();

        // 1. Point cache dir to this isolated test directory
        init_ipadic_cache_dir(test_base.clone());

        // Path 1: Direct scrub_plain / scrub_text
        let input_1 = "佐藤さんは来週東京本社に行きます。";
        let out_1 = scrub_text(input_1);
        assert!(out_1.text.contains("[NAME]"), "Direct scrub_text must redact Japanese surname: got {}", out_1.text);

        // Path 2: scrub_text_with with options (as used in scrub_for_scan)
        let opts = ScrubOptions::default();
        let out_2 = scrub_text_with("田中太郎さんと鈴木さんが会議に出席しました。", &opts);
        assert!(out_2.text.contains("[NAME]"), "scrub_text_with must redact Japanese names: got {}", out_2.text);

        // Path 3: Segmenter details and foreign words check
        assert!(is_foreign_token_in_word_list("会社", "ja"), "Common word 会社 should be recognized via segmenter");

        // Verify cache directory was successfully created and populated with valid IPADIC files
        let cache_v1 = test_base.join(IPADIC_CACHE_DIR_NAME);
        assert!(cache_v1.exists(), "Cache dir {:?} must exist after Japanese scrub", cache_v1);
        assert!(is_ipadic_cache_valid(&cache_v1), "Cache dir must be valid with all nine uncompressed files");

        let _ = std::fs::remove_dir_all(&test_base);
    }

    #[test]
    fn test_all_manifest_sha256_checksums() {
        use sha2::{Digest, Sha256};

        fn collect_files(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
            for entry in std::fs::read_dir(dir).expect("read resource dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    collect_files(base, &path, out);
                } else if path.is_file() {
                    let rel = path
                        .strip_prefix(base)
                        .expect("strip prefix")
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.insert(rel);
                }
            }
        }

        let root = resolve_resource_root().expect("resolve resource root");
        for spec in LANGUAGE_MANIFESTS {
            let manifest = load_language_manifest(&root, spec.lang)
                .unwrap_or_else(|e| panic!("Failed to load manifest for {}: {}", spec.lang, e));
            assert!(!manifest.files.is_empty(), "{} manifest must list files", spec.lang);

            let lang_dir = root.join(spec.lang);
            let mut manifested_paths = BTreeSet::new();
            for entry in &manifest.files {
                assert!(
                    manifested_paths.insert(entry.path.clone()),
                    "Duplicate path '{}' in {}/manifest.json",
                    entry.path,
                    spec.lang
                );
                let file_path = lang_dir.join(&entry.path);
                assert!(
                    file_path.is_file(),
                    "Manifest entry '{}/{}' missing on disk at {:?}",
                    spec.lang,
                    entry.path,
                    file_path
                );
                let bytes = std::fs::read(&file_path)
                    .unwrap_or_else(|e| panic!("Read '{}/{}': {}", spec.lang, entry.path, e));
                assert!(
                    !bytes.is_empty(),
                    "Manifest entry '{}/{}' must not be empty",
                    spec.lang,
                    entry.path
                );
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let actual_sha256 = format!("{:x}", hasher.finalize());
                assert_eq!(
                    actual_sha256, entry.sha256,
                    "SHA-256 mismatch for '{}/{}'",
                    spec.lang, entry.path
                );
            }

            let mut disk_files = BTreeSet::new();
            collect_files(&lang_dir, &lang_dir, &mut disk_files);
            for rel in disk_files {
                let fname = Path::new(&rel)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if fname == "manifest.json"
                    || fname.starts_with("LICENSE_")
                    || fname.starts_with("NOTICE_")
                {
                    continue;
                }
                assert!(
                    manifested_paths.contains(&rel),
                    "Unmanifested file on disk: '{}/{}'",
                    spec.lang,
                    rel
                );
            }
        }
    }

    #[test]
    fn test_missing_or_corrupt_manifest_file_returns_err() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let bad_root = std::env::temp_dir().join(format!(
            "test_missing_corrupt_manifest_{}_{}",
            std::process::id(),
            unique_id
        ));
        let _ = std::fs::remove_dir_all(&bad_root);
        std::fs::create_dir_all(bad_root.join("de")).unwrap();

        // 1. Missing manifest.json
        assert!(load_language_manifest(&bad_root, "de").is_err());

        // 2. Corrupt JSON manifest.json
        std::fs::write(bad_root.join("de/manifest.json"), "{not valid json").unwrap();
        let corrupt_err = load_language_manifest(&bad_root, "de").unwrap_err();
        assert!(corrupt_err.contains("corrupt manifest JSON"), "got: {}", corrupt_err);

        // 3. Valid manifest pointing to missing file
        std::fs::write(
            bad_root.join("de/manifest.json"),
            r#"{"language":"de","files":[{"path":"de_names.txt","role":"name_list","source":"t","licence":"t","version":"1","sha256":"0"}]}"#,
        )
        .unwrap();
        let missing_err = read_manifest_file_string_from_root(&bad_root, "de", "de_names.txt").unwrap_err();
        assert!(missing_err.contains("missing or unreadable"), "got: {}", missing_err);

        // 4. Empty (0-byte) file returns Err rather than empty set
        std::fs::write(bad_root.join("de/de_names.txt"), "").unwrap();
        let empty_err = read_manifest_file_string_from_root(&bad_root, "de", "de_names.txt").unwrap_err();
        assert!(empty_err.contains("empty"), "got: {}", empty_err);

        // 5. Missing fr / pt / hi files return Err rather than empty sets
        assert!(load_fr_resources_from_root(&bad_root).is_err());
        assert!(load_pt_resources_from_root(&bad_root).is_err());
        assert!(load_hi_resources_from_root(&bad_root).is_err());

        let _ = std::fs::remove_dir_all(&bad_root);
    }

    #[test]
    fn test_all_language_lazy_load_timings() {
        let t_startup = std::time::Instant::now();
        let startup_reads = snapshot_recorded_manifest_reads().len();
        let startup_elapsed = t_startup.elapsed();

        eprintln!(
            "[lazy_timing] startup_check={:?} manifest_reads_before_first_scan={}",
            startup_elapsed, startup_reads
        );

        let samples: &[(&str, &str)] = &[
            ("en", "Contact: Alice Johnson reviewed the annual report yesterday."),
            ("es", "El señor Carlos Mendoza firmó el contrato en Madrid ayer."),
            ("fr", "Bonjour Julien Moreau, le document a été signé aujourd'hui à Paris."),
            ("pt", "O senhor João Silva entregou o relatório em Lisboa ontem."),
            ("de", "Herr Klaus Müller arbeitet in der Abteilung seit Montag."),
            ("it", "Il signore Marco Rossi ha firmato il contratto ieri mattina."),
            ("nl", "De heer Pieter Jansen heeft het document vandaag ondertekend."),
            ("ja", "佐藤太郎さんは昨日東京の本社で会議に出席しました。"),
            ("ko", "김민수 씨는 어제 서울 사무실에서 회의를 진행했습니다."),
            ("bn", "গতকাল মুহাম্মদ ইউনূস একটি নতুন বই কিনেছেন।"),
            ("hi", "कल रोहित शर्मा ने दिल्ली में एक नया बयान जारी किया।"),
        ];

        for &(lang, sample) in samples {
            let before_reads = snapshot_recorded_manifest_reads();
            let t0 = std::time::Instant::now();
            let rep1 = scrub_plain(sample);
            let cold_elapsed = t0.elapsed();
            assert!(rep1.error.is_none(), "{} scrub error: {:?}", lang, rep1.error);

            let after_reads = snapshot_recorded_manifest_reads();
            let newly_opened: Vec<String> = after_reads.difference(&before_reads).cloned().collect();

            let t1 = std::time::Instant::now();
            let rep2 = scrub_plain(sample);
            let warm_elapsed = t1.elapsed();
            assert!(rep2.error.is_none());

            eprintln!(
                "[lazy_timing] lang={} cold={:?} warm={:?} newly_opened_files={}",
                lang,
                cold_elapsed,
                warm_elapsed,
                newly_opened.join(",")
            );
        }
    }

    #[test]
    fn test_short_code_labels_redacted_digit_run_only() {
        let cases = [
            ("CVC: 123", "CVC: [ID_NUMBER]"),
            ("CVV: 456", "CVV: [ID_NUMBER]"),
            ("CVV2: 789", "CVV2: [ID_NUMBER]"),
            ("CSC: 1234", "CSC: [ID_NUMBER]"),
            ("PIN 4821", "PIN [ID_NUMBER]"),
            ("Your PIN is 4821.", "Your PIN is [ID_NUMBER]."),
            ("OTP 90217", "OTP [ID_NUMBER]"),
            ("one-time code: 654321", "one-time code: [ID_NUMBER]"),
            ("verification code is 847291", "verification code is [ID_NUMBER]"),
            ("passcode: 11223344", "passcode: [ID_NUMBER]"),
            ("security code 987", "security code [ID_NUMBER]"),
            ("access code: 4455", "access code: [ID_NUMBER]"),
            // Multilingual PIN labels across all supported languages
            ("código PIN: 4821", "código PIN: [ID_NUMBER]"),
            ("code PIN: 4821", "code PIN: [ID_NUMBER]"),
            ("código de verificación: 4821", "código de verificación: [ID_NUMBER]"),
            ("पिन: 4821", "पिन: [ID_NUMBER]"),
            ("暗証番号: 4821", "暗証番号: [ID_NUMBER]"),
            ("暗証番号：4821", "暗証番号：[ID_NUMBER]"),
            ("비밀번호: 4821", "비밀번호: [ID_NUMBER]"),
            ("পিন: 4821", "পিন: [ID_NUMBER]"),
        ];
        for (input, expected) in cases {
            assert_eq!(scrub_plain(input).text, expected, "input: {}", input);
        }
    }

    #[test]
    fn test_short_code_labels_negative_and_bounds() {
        let negative_cases = [
            "enter your PIN on the next screen",
            "PIN",
            "in 2024",
            "Your PIN was updated back in 2024",
            "PIN in 2024",
            "costs 350 dollars",
            "PIN costs 350 dollars",
            "PIN: $350",
            "PIN: 12",
            "PIN: 3.50",
        ];
        for input in negative_cases {
            assert_eq!(scrub_plain(input).text, input, "false positive on: {}", input);
        }

        // Verify short_code_span enforces the 3..=8 digit length bound directly.
        assert_eq!(short_code_span("12", 0), None);
        assert_eq!(short_code_span("123", 0), Some((0, 3)));
        assert_eq!(short_code_span("12345678", 0), Some((0, 8)));
        assert_eq!(short_code_span("123456789", 0), None);
    }
}
