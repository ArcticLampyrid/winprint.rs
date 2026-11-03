//! PrintTicket/DEVMODE round trips through the in-box generic PostScript driver.
//! Temporary file-port queues require no hardware and receive no print jobs.

#![cfg(all(windows, feature = "test-utils"))]

use winprint::{
    test_utils::file_device::{FilePrinterDevice, PublisherColor},
    ticket::{
        document::{reader::ParsableXmlDocument, PrintTicketDocument, NS_PSK},
        FeatureOptionPackWithPredefined, PredefinedPageColorManagement,
        PredefinedPageIcmRenderingIntent, PrintCapabilities, PrintTicket, PrintTicketBuilder,
    },
};

fn assert_selections(ticket: &PrintTicket, selections: &[(&str, &str)]) {
    let document =
        PrintTicketDocument::parse_from_bytes(ticket.get_xml()).expect("parse validated ticket");
    for &(feature_name, option_name) in selections {
        let features = document
            .features
            .iter()
            .filter(|feature| {
                feature.name.namespace_ref() == Some(NS_PSK)
                    && feature.name.local_name == feature_name
            })
            .collect::<Vec<_>>();
        assert_eq!(features.len(), 1, "feature {feature_name}");
        assert_eq!(features[0].options.len(), 1, "options of {feature_name}");
        let name = features[0].options[0].name.as_ref().expect("named option");
        assert_eq!(
            (name.namespace_ref(), name.local_name.as_str()),
            (Some(NS_PSK), option_name),
            "selection of {feature_name}",
        );
    }
}

#[test]
fn color_management_and_intents_survive_validation_and_devmode_round_trip() {
    use PredefinedPageColorManagement as Management;
    use PredefinedPageIcmRenderingIntent as Intent;

    let device = FilePrinterDevice::<PublisherColor>::new()
        .expect("install in-box MS Publisher Color Printer");
    let device = device.device();
    let capabilities = PrintCapabilities::fetch(device).expect("fetch printer capabilities");

    // Require every predefined option. Empty iterators or missing options must fail.
    let managements = [
        (Management::None, "None"),
        (Management::System, "System"),
        (Management::Driver, "Driver"),
        (Management::Device, "Device"),
    ]
    .map(|(predefined, name)| {
        let pack = capabilities
            .page_color_managements()
            .find(|option| option.as_predefined_name() == Some(predefined))
            .unwrap_or_else(|| panic!("driver must advertise PageColorManagement/{name}"));
        (pack, name)
    });
    let intents = [
        (Intent::AbsoluteColorimetric, "AbsoluteColorimetric"),
        (Intent::RelativeColorimetric, "RelativeColorimetric"),
        (Intent::Photographs, "Photographs"),
        (Intent::BusinessGraphics, "BusinessGraphics"),
    ]
    .map(|(predefined, name)| {
        let pack = capabilities
            .page_icm_rendering_intents()
            .find(|option| option.as_predefined_name() == Some(predefined))
            .unwrap_or_else(|| panic!("driver must advertise PageICMRenderingIntent/{name}"));
        (pack, name)
    });

    // Check all 16 pairs so merging an intent cannot silently reset color management.
    for (management, management_name) in &managements {
        for (intent, intent_name) in &intents {
            let selections = [
                ("PageColorManagement", *management_name),
                ("PageICMRenderingIntent", *intent_name),
            ];
            let mut builder = PrintTicketBuilder::new(device).expect("create ticket builder");
            builder
                .merge(management.clone())
                .expect("merge color management");
            builder
                .merge(intent.clone())
                .expect("merge rendering intent");
            let ticket = builder.build().expect("build validated ticket");
            assert_selections(&ticket, &selections);

            let devmode = ticket
                .to_dev_mode(device)
                .expect("convert ticket to DEVMODE");
            let round_trip = PrintTicket::from_dev_mode(device, &devmode)
                .expect("convert DEVMODE back to ticket");
            assert_selections(&round_trip, &selections);
        }
    }
}
