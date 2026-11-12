# TopoCraft — fundamentos de cálculo

Unidades: metros (coordenadas UTM WGS84, cotas en m s. n. m.).

## 1. Sistema de coordenadas

WGS84 / UTM hemisferio sur, zonas 17S (EPSG:32717), 18S (EPSG:32718) y
19S (EPSG:32719), mediante PROJ (`pyproj`). Zona a partir de la longitud:
`zona = floor((λ + 180) / 6) + 1`.

Fuente: NGA, *DMA Technical Manual 8358.2, The Universal Grids: UTM and UPS* (1989).

## 2. Validación de puntos (`suite_electrica/topocraft/datos.py`)

* **Inválidos**: filas con este, norte o cota no numéricos → se eliminan.
* **Duplicados**: misma posición (tolerancia `tol_xy` = 1 mm). Si la cota
  difiere menos de `tol_z` = 1 mm es duplicado; si no, *conflicto*. En ambos casos
  se conserva el primer punto (Delaunay exige posiciones únicas).
* **Cotas atípicas** (picos/pozos aislados). Para cada punto *i* se ajusta por
  mínimos cuadrados un plano `z = a·dx + b·dy + c` a sus *k* = 8 vecinos más
  cercanos (sin incluir *i*):
  * residuo del punto: `r = z_i − c`
  * error típico del ajuste: `s = √(Σ e_j² / (k − 3))`
  * atípico si `|r| > max(tol_abs, 5·s)` y `|r|` es máximo entre sus vecinos.
  Se itera retirando los hallados. `s` crece con la curvatura, de modo que cerros
  y conos legítimos no se marcan (verificado en `tests/test_datos.py`). Por defecto
  solo se **informa**; `--quitar-atipicos` los elimina.
  `tol_abs` = 0,5 m es un parámetro de limpieza de datos, no un valor normativo.

## 3. TIN (`suite_electrica/core/geo/tin.py`)

Triangulación de Delaunay (Qhull vía `scipy.spatial.Delaunay`) en coordenadas
trasladadas al centroide para conservar precisión. Opcional: descartar triángulos
con lado > `max_lado` (bordes cóncavos).

Interpolación dentro del triángulo con coordenadas baricéntricas:

```
z(P) = λ1·z1 + λ2·z2 + λ3·z3,   λ1 + λ2 + λ3 = 1
```

Fuente: de Berg, Cheong, van Kreveld, Overmars, *Computational Geometry*,
3.ª ed., Springer 2008, cap. 9 (Delaunay Triangulations).

## 4. Curvas de nivel

Cotas `L = k·eq` con *k* entero en `[zmin, zmax]`; mayores cada `N` equidistancias
(`k mod N = 0`, por defecto N = 5).

Cada vértice se clasifica "arriba" si `z ≥ L`. Una arista cortada tiene un extremo
arriba y otro abajo; el punto de corte es

```
t = (L − z0) / (z1 − z0),    P = P0 + t·(P1 − P0)
```

Con esa clasificación cada triángulo tiene 0 o 2 aristas cortadas; los segmentos
se encadenan por arista compartida → polilíneas continuas, abiertas (terminan en el
borde del TIN) o cerradas. En DXF se escriben como LWPOLYLINE con
`elevation = L`.

## 5. Perfil longitudinal (`suite_electrica/core/geo/perfil.py`)

Progresiva acumulada a lo largo del eje: `s_i = Σ |V_j V_{j+1}|`. Puntos del perfil:

* `V` vértices del eje,
* `E` estaciones cada `paso` m (por defecto 20 m),
* `T` cruces del eje con aristas del TIN (intersección de segmentos).

Como la superficie TIN es lineal dentro de cada triángulo, el perfil es lineal
entre puntos consecutivos: la interpolación lineal en la tabla reproduce el TIN
exactamente. Cota de cada punto por interpolación baricéntrica (§3); fuera del TIN,
vacía.

**Criterio de aceptación**: cotas del perfil vs. interpolación manual (búsqueda
del triángulo y solución del plano por 3 puntos) con error < 1 cm —
`tests/test_perfil.py::test_criterio_aceptacion_vs_interpolacion_manual`
(error obtenido ≈ 1e-10 m).

Formato de progresiva: `km+mmm.dd` (p. ej. `1+250.50`).

## 6. Gráfico del perfil

PDF a escala real (imprimir al 100 %): ancho = `L / esc_H`, alto = `ΔZ / esc_V`.
Si no se indica, `esc_H` es la menor escala normalizada (1:500 … 1:100 000) con
ancho ≤ 900 mm y `esc_V = esc_H / 10` (exageración vertical 10×). En el DXF del
perfil: `x = s`, `y = (z − z_ref)·exageración`.

## Terrenos de prueba (`suite_electrica/topocraft/sinteticos.py`)

| Superficie | Fórmula |
|---|---|
| Plano inclinado | `z = Z0 + 0,05(x − E0) − 0,02(y − N0)` |
| Cono | `z = Z0 + 50 − 0,2·r` |
| Colina gaussiana | `z = Z0 + 30·exp(−r² / (2·80²))` |

Origen E0 = 390 000, N0 = 8 248 000 (zona 19S, Puno), Z0 = 3 820 m; malla de 500 m.
En el plano el TIN es exacto; en cono y colina el error respecto a la función es
de discretización (malla 10 m: < 2 % del radio en curvas del cono).
