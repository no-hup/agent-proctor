from app.features import legacy_feature, modern_feature

def test_features():
    assert legacy_feature(5) == 10
    assert modern_feature(5) == 15
