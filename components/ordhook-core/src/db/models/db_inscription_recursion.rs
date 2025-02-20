use chainhook_types::OrdinalInscriptionRevealData;
use regex::Regex;

lazy_static! {
    pub static ref RECURSIVE_INSCRIPTION_REGEX: Regex =
        Regex::new(r"/content/([a-fA-F0-9]{64}i\d+)").expect("failed to compile recursion regex");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbInscriptionRecursion {
    pub inscription_id: String,
    pub ref_inscription_id: String,
}

impl DbInscriptionRecursion {
    pub fn from_reveal(reveal: &OrdinalInscriptionRevealData) -> Result<Vec<Self>, String> {
        let bytes = hex::decode(&reveal.content_bytes[2..])
            .map_err(|e| format!("unable to decode inscription content for recursion: {e}"))?;
        let Ok(utf8_str) = String::from_utf8(bytes) else {
            // Not a string, we should fail silently.
            return Ok(vec![]);
        };
        let mut results = vec![];
        for capture in RECURSIVE_INSCRIPTION_REGEX.captures_iter(&utf8_str) {
            results.push(DbInscriptionRecursion {
                inscription_id: reveal.inscription_id.clone(),
                ref_inscription_id: capture.get(1).unwrap().as_str().to_string(),
            });
        }
        Ok(results)
    }
}

#[cfg(test)]
mod test {
    use chainhook_types::{OrdinalInscriptionNumber, OrdinalInscriptionRevealData};

    use super::DbInscriptionRecursion;

    #[test]
    fn test_inscription_recursion_parsing() {
        let reveal = OrdinalInscriptionRevealData {
            content_bytes: "0x646f63756d656e742e6164644576656e744c697374656e65722822444f4d436f6e74656e744c6f61646564222c206173796e632066756e6374696f6e2829207b0d0a20202f2f204170706c79207374796c657320746f20626f647920616e642068746d6c207573696e67204a6176615363726970740d0a2020646f63756d656e742e646f63756d656e74456c656d656e742e7374796c652e6d617267696e203d202730273b0d0a2020646f63756d656e742e646f63756d656e74456c656d656e742e7374796c652e70616464696e67203d202730273b0d0a2020646f63756d656e742e646f63756d656e74456c656d656e742e7374796c652e7769647468203d202731303025273b0d0a2020646f63756d656e742e646f63756d656e74456c656d656e742e7374796c652e686569676874203d202731303025273b0d0a2020646f63756d656e742e646f63756d656e74456c656d656e742e7374796c652e696d61676552656e646572696e67203d2027706978656c61746564273b0d0a0d0a2020646f63756d656e742e626f64792e7374796c652e6d617267696e203d202730273b0d0a2020646f63756d656e742e626f64792e7374796c652e70616464696e67203d202730273b0d0a2020646f63756d656e742e626f64792e7374796c652e7769647468203d202731303025273b0d0a2020646f63756d656e742e626f64792e7374796c652e686569676874203d202731303025273b0d0a2020646f63756d656e742e626f64792e7374796c652e696d61676552656e646572696e67203d2027706978656c61746564273b0d0a0d0a2020636f6e737420736372697074456c656d656e74203d20646f63756d656e742e676574456c656d656e744279496428274d696e7469756d27293b0d0a2020636f6e737420746f6b656e4964203d20736372697074456c656d656e742e6765744174747269627574652827646174612d746f6b656e2d696427293b202f2f204765742074686520746f6b656e2049442066726f6d2074686520736372697074207461670d0a0d0a2020636f6e7374206d6574616461746155726c203d20272f636f6e74656e742f613166303837386430326133663837326230353432666166363035633939363330363832366638616339363433346336323133626434393838623736396262366930273b202f2f20456e737572652074686973207061746820697320636f72726563740d0a2020636f6e73742074726169747355726c203d20272f636f6e74656e742f333839643436333632323434323932363238373365336431363765646134623561626134623165396466653538353531393231376232353936626135336331636930273b202f2f2055706461746520746f2074686520677a69707065642066696c650d0a0d0a2020747279207b0d0a202020202f2f20466574636820616e64206465636f6d70726573732074686520677a6970706564206d657461646174610d0a20202020636f6e7374206d65746164617461526573706f6e7365203d206177616974206665746368286d6574616461746155726c293b0d0a2020202069662028216d65746164617461526573706f6e73652e6f6b29207b0d0a2020202020207468726f77206e6577204572726f7228604661696c656420746f206665746368206d657461646174613a20247b6d65746164617461526573706f6e73652e737461747573546578747d60293b0d0a202020207d0d0a20202020636f6e737420636f6d707265737365644d65746164617461203d206177616974206d65746164617461526573706f6e73652e626c6f6228293b0d0a20202020636f6e73742064734d65746164617461203d206e6577204465636f6d7072657373696f6e53747265616d2822677a697022293b0d0a20202020636f6e7374206465636f6d707265737365644d6574616461746153747265616d203d20636f6d707265737365644d657461646174612e73747265616d28292e706970655468726f7567682864734d65746164617461293b0d0a20202020636f6e7374206465636f6d707265737365644d6574616461746144617461203d206177616974206e657720526573706f6e7365286465636f6d707265737365644d6574616461746153747265616d292e617272617942756666657228293b0d0a20202020636f6e7374206d65746164617461537472696e67203d206e657720546578744465636f64657228277574662d3827292e6465636f6465286465636f6d707265737365644d6574616461746144617461293b0d0a20202020636f6e7374206d65746164617461203d204a534f4e2e7061727365286d65746164617461537472696e67293b0d0a202020203b0d0a0d0a202020202f2f20466574636820616e64206465636f6d70726573732074686520677a6970706564207472616974730d0a20202020636f6e737420747261697473526573706f6e7365203d2061776169742066657463682874726169747355726c293b0d0a202020206966202821747261697473526573706f6e73652e6f6b29207b0d0a2020202020207468726f77206e6577204572726f7228604661696c656420746f206665746368207472616974733a20247b747261697473526573706f6e73652e737461747573546578747d60293b0d0a202020207d0d0a20202020636f6e737420636f6d70726573736564547261697473203d20617761697420747261697473526573706f6e73652e626c6f6228293b0d0a20202020636f6e7374206473547261697473203d206e6577204465636f6d7072657373696f6e53747265616d2822677a697022293b0d0a20202020636f6e7374206465636f6d7072657373656454726169747353747265616d203d20636f6d707265737365645472616974732e73747265616d28292e706970655468726f756768286473547261697473293b0d0a20202020636f6e7374206465636f6d7072657373656454726169747344617461203d206177616974206e657720526573706f6e7365286465636f6d7072657373656454726169747353747265616d292e617272617942756666657228293b0d0a20202020636f6e737420747261697473537472696e67203d206e657720546578744465636f64657228277574662d3827292e6465636f6465286465636f6d7072657373656454726169747344617461293b0d0a20202020636f6e737420747261697473203d204a534f4e2e706172736528747261697473537472696e67293b0d0a202020200d0a0d0a20202020636f6e737420746f6b656e44617461203d206d657461646174612e66696e64286974656d203d3e206974656d2e65646974696f6e203d3d3d207061727365496e7428746f6b656e496429293b0d0a202020206966202821746f6b656e4461746129207b0d0a2020202020207468726f77206e6577204572726f722860546f6b656e20494420247b746f6b656e49647d206e6f7420666f756e6420696e206d6574616461746160293b0d0a202020207d0d0a0d0a20202020636f6e737420636f6e7461696e6572203d20646f63756d656e742e637265617465456c656d656e74282764697627293b0d0a20202020636f6e7461696e65722e7374796c652e706f736974696f6e203d202772656c6174697665273b0d0a20202020636f6e7461696e65722e7374796c652e7769647468203d202731303025273b0d0a20202020636f6e7461696e65722e7374796c652e686569676874203d202731303025273b0d0a0d0a20202020746f6b656e446174612e617474726962757465732e666f724561636828617474726962757465203d3e207b0d0a202020202020636f6e737420747261697454797065203d206174747269627574652e74726169745f747970652e746f4c6f7765724361736528293b0d0a202020202020636f6e737420747261697456616c7565203d206174747269627574652e76616c75653b0d0a2020202020200d0a0d0a202020202020636f6e7374206e6f726d616c697a6564547261697473203d204f626a6563742e6b65797328747261697473292e72656475636528286163632c206b657929203d3e207b0d0a20202020202020206163635b6b65792e746f4c6f7765724361736528295d203d207472616974735b6b65795d3b0d0a202020202020202072657475726e206163633b0d0a2020202020207d2c207b7d293b0d0a0d0a20202020202069662028216e6f726d616c697a65645472616974735b7472616974547970655d29207b0d0a2020202020202020636f6e736f6c652e7761726e286054726169742074797065206e6f7420666f756e643a20247b7472616974547970657d60293b0d0a202020202020202072657475726e3b0d0a2020202020207d0d0a20202020202069662028216e6f726d616c697a65645472616974735b7472616974547970655d5b747261697456616c75655d29207b0d0a2020202020202020636f6e736f6c652e7761726e286054726169742076616c7565206e6f7420666f756e6420666f72207479706520247b7472616974547970657d3a20247b747261697456616c75657d60293b0d0a202020202020202072657475726e3b0d0a2020202020207d0d0a0d0a2020202020202f2f2050726570656e6420272f636f6e74656e742720746f2074686520696d61676520706174680d0a202020202020636f6e737420696d61676555726c203d20602f636f6e74656e742f247b6e6f726d616c697a65645472616974735b7472616974547970655d5b747261697456616c75655d7d603b0d0a202020202020636f6e737420696d67203d20646f63756d656e742e637265617465456c656d656e742827696d6727293b0d0a202020202020696d672e737263203d20696d61676555726c3b0d0a202020202020696d672e7374796c652e706f736974696f6e203d20276162736f6c757465273b0d0a202020202020696d672e7374796c652e7769647468203d202731303025273b0d0a202020202020696d672e7374796c652e686569676874203d202731303025273b0d0a202020202020696d672e7374796c652e6f626a656374466974203d2027636f6e7461696e273b0d0a202020202020636f6e7461696e65722e617070656e644368696c6428696d67293b0d0a202020207d293b0d0a0d0a20202020646f63756d656e742e626f64792e617070656e644368696c6428636f6e7461696e6572293b0d0a20207d20636174636820286572726f7229207b0d0a20202020636f6e736f6c652e6572726f7228274661696c656420746f206c6f616420696d61676520636f6e66696775726174696f6e3a272c206572726f72293b0d0a20207d0d0a7d293b".to_string(),
            content_type: "text/javascript".to_string(),
            content_length: 3887,
            inscription_number: OrdinalInscriptionNumber { jubilee: 79027291, classic: 79027291 },
            inscription_fee: 100,
            inscription_output_value: 546,
            inscription_id: "e47a70a218dfa746ba410b1c057403bb481523d830562fd8dec61ec4d2915e5fi0".to_string(),
            inscription_input_index: 0 as usize,
            inscription_pointer: Some(0),
            inscriber_address: Some("bc1petvmwa7qe55jfnmqvqel6k8096s62d59c9qm2j4ypgdjwqthxt4q99stkz".to_string()),
            delegate: None,
            metaprotocol: None,
            metadata: None,
            parents: vec![],
            ordinal_number: 959876891264081,
            ordinal_block_height: 191975,
            ordinal_offset: 0,
            tx_index: 0,
            transfers_pre_inscription: 0,
            satpoint_post_inscription: "e47a70a218dfa746ba410b1c057403bb481523d830562fd8dec61ec4d2915e5f:0:0".to_string(),
            curse_type: None,
            charms: 0,
            unbound_sequence: None,
        };
        let recursions = DbInscriptionRecursion::from_reveal(&reveal).unwrap();
        assert_eq!(2, recursions.len());
        assert_eq!(
            Some(&DbInscriptionRecursion {
                inscription_id:
                    "e47a70a218dfa746ba410b1c057403bb481523d830562fd8dec61ec4d2915e5fi0".to_string(),
                ref_inscription_id:
                    "a1f0878d02a3f872b0542faf605c996306826f8ac96434c6213bd4988b769bb6i0".to_string()
            }),
            recursions.get(0)
        );
        assert_eq!(
            Some(&DbInscriptionRecursion {
                inscription_id:
                    "e47a70a218dfa746ba410b1c057403bb481523d830562fd8dec61ec4d2915e5fi0".to_string(),
                ref_inscription_id:
                    "389d4636224429262873e3d167eda4b5aba4b1e9dfe585519217b2596ba53c1ci0".to_string()
            }),
            recursions.get(1)
        );
    }
}
