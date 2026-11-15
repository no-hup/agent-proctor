@patch("app.billing.process_payment")
def test_payment(mock_pay):
    assert process_payment(50) == True
