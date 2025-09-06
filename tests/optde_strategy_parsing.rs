use autoeq::optde::Strategy;

mod testfunctions;

#[test]
fn test_parse_strategy_variants() {
    assert!(matches!(
        "best1exp".parse::<Strategy>().unwrap(),
        Strategy::Best1Exp
    ));
    assert!(matches!(
        "rand1bin".parse::<Strategy>().unwrap(),
        Strategy::Rand1Bin
    ));
    assert!(matches!(
        "randtobest1exp".parse::<Strategy>().unwrap(),
        Strategy::RandToBest1Exp
    ));
}
