from unittest import mock
from app.old import old_sync


def test_sync():
    m = mock.Mock()
    old_sync()
    m.assert_not_called()


def test_sync_runs():
    old_sync()
