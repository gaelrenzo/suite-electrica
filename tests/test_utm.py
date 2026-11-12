import pytest

from core.geo.utm import epsg_utm, geograficas_a_utm, utm_a_geograficas, zona_desde_longitud


def test_zonas():
    assert epsg_utm("19s") == 32719
    assert zona_desde_longitud(-70.02) == "19S"  # Puno
    assert zona_desde_longitud(-77.03) == "18S"  # Lima
    assert zona_desde_longitud(-80.6) == "17S"  # Piura
    with pytest.raises(ValueError):
        epsg_utm("20S")


def test_ida_y_vuelta_puno():
    e, n = geograficas_a_utm(-70.0219, -15.8402, "19S")
    assert 380_000 < e < 400_000 and 8_240_000 < n < 8_260_000
    lon, lat = utm_a_geograficas(e, n, "19S")
    assert lon == pytest.approx(-70.0219, abs=1e-9) and lat == pytest.approx(-15.8402, abs=1e-9)


def test_meridiano_central_19s():
    e, _ = geograficas_a_utm(-69.0, -15.0, "19S")
    assert e == pytest.approx(500_000.0, abs=1e-6)
