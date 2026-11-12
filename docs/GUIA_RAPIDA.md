# Guía rápida

## 1. Instalar

**Windows 10/11**

1. Instale Python 3.12 o superior desde <https://www.python.org/downloads/> y
   marque **"Add python.exe to PATH"**.
2. Descargue el repositorio (`git clone git@github.com:gaelrenzo/suite-electrica.git`
   o *Code → Download ZIP*).
3. Haga doble clic en **`instalar.bat`**. Crea `.venv`, instala todo, ejecuta las
   pruebas y agrega `bin\` al PATH del usuario.
4. Abra una **nueva** terminal (PowerShell o CMD) y pruebe `topocraft --help`.

**Linux / Termux-Ubuntu**

```bash
cd suite-electrica
python3 -m pip install -e ".[dev]"   # en Debian/Ubuntu del sistema: añadir --break-system-packages
topocraft --help
```

## 2. Topografía (TopoCraft)

Prepare el levantamiento en CSV (UTM WGS84, metros):

```
punto,este,norte,cota,descripcion
1,412606.01,8270712.24,3924.17,BM
2,412620.50,8270730.10,3921.80,TN
```

También sirve con `;`, coma decimal o sin encabezado (orden P,E,N,Z,D), o un DXF
con puntos. El eje de la línea es un DXF con una polilínea (de preferencia en la
capa `EJE`) o un CSV `este,norte`.

```bash
topocraft validar puntos.csv                       # revisa duplicados y cotas atípicas
topocraft curvas  puntos.csv --eq 1 --eje eje.dxf --out plano.dxf
topocraft perfil  puntos.csv --eje eje.dxf --out perfil.pdf perfil.xlsx perfil.dxf
```

* `--quitar-atipicos` elimina las cotas atípicas (por defecto solo se informan).
* `--max-lado 50` evita triángulos largos en los bordes del levantamiento.
* `--esc-h 2000 --esc-v 200` fija las escalas del PDF (imprimir al 100 %).

## 3. Línea primaria (LineCraft)

```bash
linecraft nuevo mi_linea          # crea proyecto.json, conductores.csv, estructuras.csv
```

1. Copie `puntos.csv` y `eje.dxf` a `mi_linea/` (o en `proyecto.json` use
   `"perfil": {"excel": "perfil.xlsx"}` con el Excel de TopoCraft).
2. Edite `proyecto.json`: conductor, hipótesis, EDS, tiro máximo, distancia
   mínima, altura de amarre, vano máximo **y la fuente de cada valor** en `fuentes`.
   Los valores de la plantilla vienen del proyecto Capachica y están marcados
   PENDIENTE: verifíquelos contra el CNE y las normas DGE.
3. Verifique `conductores.csv` con la ficha del fabricante.
4. Calcule:

```bash
linecraft calcular mi_linea/proyecto.json --out mi_linea/salidas
```

Salidas en `mi_linea/salidas/`:

| Archivo | Contenido |
|---|---|
| `perfil_linea.dxf` | Terreno, postes, conductor con flecha máxima y línea de despeje mínimo |
| `planilla_estructuras.xlsx` | Estructuras, vanos/flechas/despejes, cálculo mecánico, cargas, avisos |
| `memoria_calculo.pdf` | Memoria con datos, fuentes, fórmulas, perfil y tablas |

La consola indica **CUMPLE / NO CUMPLE** y los avisos. Para fijar los postes a
mano, llene `estructuras.csv` (incluya 0, el final y todos los vértices del eje)
y ponga `"estructuras": "estructuras.csv"` en `proyecto.json`.

## 4. Ejemplos incluidos

```bash
python examples/generar_ejemplos.py
topocraft curvas examples/colina_gaussiana.csv --eq 1 --eje examples/eje.dxf --out examples/salidas/plano.dxf
linecraft calcular examples/linecraft/proyecto.json --out examples/salidas/linecraft
```

## 5. Problemas frecuentes

| Mensaje | Solución |
|---|---|
| `Faltan columnas` | Revise el encabezado del CSV (`este`, `norte`, `cota`). |
| `No se encontró polilínea de eje` | El DXF del eje debe tener una LWPOLYLINE o POLYLINE. |
| `vértice del eje ... no tiene estructura` | En la ubicación manual incluya todos los vértices. |
| `sale del modelo de terreno` | El eje sale del área levantada: amplíe el levantamiento. |
| `topocraft` no se reconoce (Windows) | Abra una terminal nueva después de `instalar.bat`. |
