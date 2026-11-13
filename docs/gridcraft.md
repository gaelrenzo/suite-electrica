# GridCraft — redes de distribución (fundamentos)

Equivalente libre del cálculo eléctrico de REDCAD: red **radial** MT o BT,
demandas, caída de tensión, pérdidas y verificación del transformador.

## Entradas (`red.json`)

* `nodos.csv`: `nodo, padre, longitud_m, conductor, lotes, kw_ce, lamparas, este, norte`.
  Una sola raíz (subestación, `padre` vacío). Cada fila describe el tramo
  `padre → nodo`. Con este/norte en todos los nodos se genera `plano_red.dxf`.
* `conductores_bt.csv`: `formacion, k_ohm_km, r_ohm_km, x_ohm_km, k_ap_ohm_km,
  r_ap_ohm_km, corriente_max_A, fuente`.
* Parámetros: `sistema` (`3F`/`1F`), `tension_V`, `calificacion_kw_lote`,
  `fs_sp` (o `tabla_fs` escalonada `[[abonados, fs], ...]`), `fp_sp`, `fs_ce`,
  `kw_lampara`, `fs_ap`, `fp_ap`, `caida_max_pct`, `perdidas_max_pct`,
  `factor_carga`, `transformador_kva`, `factor_sobrecarga`, `fuentes`.

## Método (sumatoria de potencias)

| Magnitud | Fórmula |
|---|---|
| Demanda SP del nodo | `lotes · CE · fs_SP + kW_CE · fs_CE` |
| Demanda AP | `lámparas · kW_lámpara · fs_AP` |
| Potencia del tramo | suma de demandas aguas abajo |
| Corriente | `I = P / (k · V · cos φ)`, `k = √3` (3φ) o 1 (1φ) |
| Factor de caída | `K` del catálogo, o `K = c (r cos φ + x sen φ)`, `c = √3` (3φ), 2 (1φ) |
| Caída del tramo | `ΔV = K · I · L[km]` (referida a `V`) |
| %ΔV del nodo | `100 · ΣΔV / V` desde la subestación |
| Pérdidas | `n_f · I² · r · L`, `n_f` = 3 (3φ), 2 (1φ) |
| Energía perdida | `P_pérd · (0,3 FC + 0,7 FC²) · 8760` |

El AP se calcula sobre su hilo (`k_ap`/`r_ap`) a la tensión fase-neutro; si el
conductor no tiene hilo de AP, su carga se suma al servicio particular.

## Alertas

Mismo criterio que LineCraft: `E = (P* − P)/P`. `DV` (%ΔV del nodo),
`DV-AP`, `I` (corriente del tramo > `corriente_max_A`), `PERD` (pérdidas %
M.D.), `TR` (kVA > `transformador_kva · factor_sobrecarga`). Límite 0 o vacío =
no se verifica. Código de salida 3 si hay alertas.

## Limitaciones

* Red radial balanceada (como REDCAD): no hay flujo de carga desbalanceado ni mallado.
* Sin autodiseño de la red ni selección automática de conductor/transformador.
* Plantilla de conductores con valores **EJEMPLO** (r = ρ/S); reemplazar por la
  tabla K de la norma DGE o la ficha del fabricante.
