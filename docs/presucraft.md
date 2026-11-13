# PresuCraft — metrados y presupuestos

Equivalente libre de PRESIO: presupuesto por **unidades constructivas** (UC).
Base de datos en CSV editables (`presucraft nuevo carpeta`); no trae precios reales.

| Tabla | Columnas |
|---|---|
| `suministros.csv` | `codigo, descripcion, unidad, precio, peso_kg, fuente` |
| `unidades.csv` | `uc, suministro, cantidad` (composición de la UC) |
| `montaje.csv` | `uc, descripcion, unidad, precio, fuente` |
| `metrado.csv` | `rubro, uc, cantidad` (LP, RP, RS, SED, CD, AP…) |

`presupuesto.json`: rutas a las tablas, `moneda`, `flete_por_kg`,
`gastos_generales_pct`, `utilidad_pct`, `igv_pct`, `fuentes`.

## Cálculo (por rubro y total)

```
Suministro = Σ n_UC · Σ q_s · precio_s
Montaje    = Σ n_UC · precio_montaje_UC
Transporte = Σ n_UC · Σ q_s · peso_s · flete_por_kg
CD = S + M + T;  GG = CD·gg%;  Utilidad = CD·u%;  Subtotal = CD + GG + U
IGV = Subtotal·igv%;  Total = Subtotal + IGV
```

Salida `presupuesto.xlsx`: Resumen, Suministro (materiales consolidados por
rubro), Montaje, PlanillaUC (costo unitario y total por UC) y Avisos.

## Integración

`presucraft metrado proyecto.sel --rubro LP --out metrado.csv` convierte el
metrado de LineCraft (postes, armados, retenidas, conductor en km) en filas de
metrado; los códigos de UC son los de los catálogos de LineCraft.

## Pendiente

Análisis de precios unitarios (APU) con recursos (mano de obra, equipos),
fórmula polinómica y flete por origen.
