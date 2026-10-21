use super::{
    define_feature_option_pack,
    document::{OwnedName, ParameterInit, PrintFeatureOption, NS_PSK},
    PredefinedPagesPerSheetDirection,
};

define_feature_option_pack!(
    vec![
        OwnedName::qualified("JobNUpAllDocumentsContiguously", NS_PSK, Some("psk")),
        OwnedName::qualified("PresentationDirection", NS_PSK, Some("psk"))
    ],
    PagesPerSheetDirection,
    PredefinedPagesPerSheetDirection
);

#[cfg(test)]
mod tests {
    use crate::{
        test_utils::null_device,
        ticket::{
            document::{OwnedName, NS_PSK},
            test_helpers::{
                assert_converted, assert_merged, capabilities_from_xml, merge_all, selected_option,
            },
            FeatureOptionPack, FeatureOptionPackWithPredefined, PagesPerSheet,
            PagesPerSheetDirection, PredefinedPagesPerSheetDirection, PrintCapabilities,
            PrintTicket, StaticFeaturePathOptionPack,
        },
    };

    #[test]
    fn use_right_bottom() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let right_bottom = capabilities
            .pages_per_sheet_directions()
            .find(|x| x.as_predefined_name() == Some(PredefinedPagesPerSheetDirection::RightBottom))
            .unwrap();
        assert_merged(&device, right_bottom);
    }

    #[test]
    fn merge_keeps_both_pages_per_sheet_and_direction() {
        let device = null_device::thread_local();
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let four_up = || {
            capabilities
                .pages_per_sheet()
                .find(|x| x.pages_per_sheet() == 4)
                .unwrap()
        };
        let bottom_left = || {
            capabilities
                .pages_per_sheet_directions()
                .find(|x| {
                    x.as_predefined_name() == Some(PredefinedPagesPerSheetDirection::BottomLeft)
                })
                .unwrap()
        };
        for deltas in [
            [PrintTicket::from(four_up()), bottom_left().into()],
            [bottom_left().into(), four_up().into()],
        ] {
            let document = merge_all(&device, deltas);
            let pages_per_sheet = PagesPerSheet::new(
                selected_option(&document, PagesPerSheet::feature_path()).clone(),
                document.parameter_inits.clone(),
            );
            let direction = PagesPerSheetDirection::new(
                selected_option(&document, PagesPerSheetDirection::feature_path()).clone(),
                vec![],
            );
            assert_eq!(pages_per_sheet.pages_per_sheet(), 4);
            assert_eq!(
                direction.as_predefined_name(),
                Some(PredefinedPagesPerSheetDirection::BottomLeft)
            );
        }
    }

    #[test]
    fn feature_paths_reach_sub_features() {
        let capabilities =
            capabilities_from_xml(include_bytes!("../../test_data/print_capabilities.xml"));
        assert_eq!(
            PagesPerSheetDirection::list(&capabilities)
                .filter_map(|x| x.option().name.clone())
                .map(|x| x.local_name)
                .collect::<Vec<_>>(),
            vec!["RightBottom", "BottomRight", "LeftBottom", "BottomLeft"]
        );
        // a path that does not go through the parent feature finds nothing
        assert_eq!(
            capabilities
                .options_for_feature(&[OwnedName::qualified(
                    "PresentationDirection",
                    NS_PSK,
                    Some("psk")
                )])
                .count(),
            0
        );
        assert_eq!(
            capabilities
                .options_for_feature(&[
                    OwnedName::qualified("JobNUpAllDocumentsContiguously", NS_PSK, Some("psk")),
                    OwnedName::qualified("PresentationDirection", NS_PSK, Some("psk"))
                ])
                .count(),
            4
        );
    }

    #[test]
    fn print_ticket_nests_sub_features() {
        let capabilities =
            capabilities_from_xml(include_bytes!("../../test_data/print_capabilities.xml"));
        let direction = PagesPerSheetDirection::list(&capabilities).next().unwrap();
        assert_eq!(
            direction.option_feature_path(),
            vec![
                OwnedName::qualified("JobNUpAllDocumentsContiguously", NS_PSK, Some("psk")),
                OwnedName::qualified("PresentationDirection", NS_PSK, Some("psk"))
            ]
        );
        assert_converted(direction);
    }
}
