from .fixtures import *


def pytest_configure():
    pytest.tracim_http_port_counter = 0
