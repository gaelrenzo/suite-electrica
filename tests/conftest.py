"""Fixtures compartidas: terrenos sintéticos con cota exacta."""

import pytest

from core.geo.tin import TIN
from topocraft import sinteticos as st


@pytest.fixture(params=["plano", "cono", "colina"])
def superficie(request):
    return {"plano": st.plano_inclinado(), "cono": st.cono(), "colina": st.colina_gaussiana()}[request.param]


@pytest.fixture
def plano():
    return st.plano_inclinado()


@pytest.fixture
def colina():
    return st.colina_gaussiana()


def tin_de(sup, paso=10.0):
    df = st.malla(sup, paso)
    return df, TIN(df[["este", "norte", "cota"]].to_numpy())
