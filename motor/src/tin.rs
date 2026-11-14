//! Modelo TIN por triangulación de Delaunay y curvas de nivel. Port de `core/geo/tin.py`.
//!
//! Interpolación baricéntrica en el triángulo; curvas por intersección plano–arista
//! ("marching triangles", vértice "arriba" si `z >= L`) encadenadas por arista compartida.

use std::collections::{BTreeMap, HashMap, HashSet};

use spade::{DelaunayTriangulation, HasPosition, Point2, PositionInTriangulation, Triangulation};

#[derive(Clone, Copy, Debug)]
struct Vertice {
    p: Point2<f64>,
    idx: usize,
}

impl HasPosition for Vertice {
    type Scalar = f64;
    fn position(&self) -> Point2<f64> {
        self.p
    }
}

/// Curva de nivel: polilínea a cota constante.
#[derive(Clone, Debug)]
pub struct Curva {
    pub cota: f64,
    pub xy: Vec<[f64; 2]>,
    pub cerrada: bool,
}

impl Curva {
    /// Longitud en planta (m).
    pub fn longitud(&self) -> f64 {
        let mut l: f64 = self.xy.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
        if self.cerrada && self.xy.len() > 1 {
            let (a, b) = (self.xy[self.xy.len() - 1], self.xy[0]);
            l += (b[0] - a[0]).hypot(b[1] - a[1]);
        }
        l
    }
}

/// Superficie TIN (coordenadas trasladadas al centroide para precisión con UTM).
pub struct Tin {
    pub xyz: Vec<[f64; 3]>,
    pub origen: [f64; 2],
    tri: DelaunayTriangulation<Vertice>,
    /// Triángulos (índices de `xyz`); `activos[k]` = cumple `max_lado`.
    pub triangulos: Vec<[usize; 3]>,
    pub activos: Vec<bool>,
    /// índice de cara de spade → índice de triángulo.
    cara_a_tri: HashMap<usize, usize>,
    /// Aristas únicas (i < j) de los triángulos activos.
    pub aristas: Vec<[usize; 2]>,
    aristas_tri: Vec<[usize; 3]>,
}

impl Tin {
    /// Triangula los puntos `(este, norte, cota)`; `max_lado` descarta triángulos largos.
    pub fn nuevo(xyz: &[[f64; 3]], max_lado: Option<f64>) -> Result<Tin, String> {
        if xyz.len() < 3 {
            return Err("Se requieren al menos 3 puntos (n, 3)".into());
        }
        let n = xyz.len() as f64;
        let origen = [xyz.iter().map(|p| p[0]).sum::<f64>() / n, xyz.iter().map(|p| p[1]).sum::<f64>() / n];
        let verts: Vec<Vertice> = xyz
            .iter()
            .enumerate()
            .map(|(i, p)| Vertice { p: Point2::new(p[0] - origen[0], p[1] - origen[1]), idx: i })
            .collect();
        let tri = DelaunayTriangulation::<Vertice>::bulk_load(verts).map_err(|e| format!("No se puede triangular: {e:?}"))?;
        if tri.num_inner_faces() == 0 {
            return Err("No se puede triangular: puntos colineales o insuficientes".into());
        }
        let mut triangulos = Vec::new();
        let mut cara_a_tri = HashMap::new();
        for f in tri.inner_faces() {
            let v = f.vertices();
            cara_a_tri.insert(f.fix().index(), triangulos.len());
            triangulos.push([v[0].data().idx, v[1].data().idx, v[2].data().idx]);
        }
        let activos: Vec<bool> = triangulos
            .iter()
            .map(|t| match max_lado {
                None => true,
                Some(m) => (0..3).all(|k| {
                    let (a, b) = (xyz[t[k]], xyz[t[(k + 1) % 3]]);
                    (a[0] - b[0]).hypot(a[1] - b[1]) <= m
                }),
            })
            .collect();
        let mut indice: BTreeMap<[usize; 2], usize> = BTreeMap::new();
        for (t, &act) in triangulos.iter().zip(&activos) {
            if act {
                for k in 0..3 {
                    let (a, b) = (t[k], t[(k + 1) % 3]);
                    indice.insert([a.min(b), a.max(b)], 0);
                }
            }
        }
        let aristas: Vec<[usize; 2]> = indice.keys().copied().collect();
        for (k, e) in aristas.iter().enumerate() {
            indice.insert(*e, k);
        }
        let aristas_tri = triangulos
            .iter()
            .zip(&activos)
            .filter(|(_, &a)| a)
            .map(|(t, _)| {
                let id = |a: usize, b: usize| indice[&[a.min(b), a.max(b)]];
                [id(t[0], t[1]), id(t[1], t[2]), id(t[2], t[0])]
            })
            .collect();
        Ok(Tin { xyz: xyz.to_vec(), origen, tri, triangulos, activos, cara_a_tri, aristas, aristas_tri })
    }

    fn baricentrica(&self, t: [usize; 3], x: f64, y: f64) -> f64 {
        let p = |i: usize| (self.xyz[i][0] - self.origen[0], self.xyz[i][1] - self.origen[1], self.xyz[i][2]);
        let ((x1, y1, z1), (x2, y2, z2), (x3, y3, z3)) = (p(t[0]), p(t[1]), p(t[2]));
        let det = (y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3);
        let l1 = ((y2 - y3) * (x - x3) + (x3 - x2) * (y - y3)) / det;
        let l2 = ((y3 - y1) * (x - x3) + (x1 - x3) * (y - y3)) / det;
        l1 * z1 + l2 * z2 + (1.0 - l1 - l2) * z3
    }

    /// Cota interpolada (m); NaN fuera del TIN o en triángulos descartados.
    pub fn cota(&self, x: f64, y: f64) -> f64 {
        let (lx, ly) = (x - self.origen[0], y - self.origen[1]);
        let caras: Vec<usize> = match self.tri.locate(Point2::new(lx, ly)) {
            PositionInTriangulation::OnFace(f) => vec![f.index()],
            PositionInTriangulation::OnEdge(e) => {
                let e = self.tri.directed_edge(e);
                [e.face(), e.rev().face()].iter().filter_map(|f| f.as_inner()).map(|f| f.fix().index()).collect()
            }
            PositionInTriangulation::OnVertex(v) => self
                .tri
                .vertex(v)
                .out_edges()
                .filter_map(|e| e.face().as_inner())
                .map(|f| f.fix().index())
                .collect(),
            _ => Vec::new(),
        };
        for c in caras {
            if let Some(&k) = self.cara_a_tri.get(&c) {
                if self.activos[k] {
                    return self.baricentrica(self.triangulos[k], lx, ly);
                }
            }
        }
        f64::NAN
    }

    /// Extremos de cada arista (coordenadas absolutas).
    pub fn segmentos_aristas(&self) -> Vec<([f64; 2], [f64; 2])> {
        self.aristas
            .iter()
            .map(|e| ([self.xyz[e[0]][0], self.xyz[e[0]][1]], [self.xyz[e[1]][0], self.xyz[e[1]][1]]))
            .collect()
    }

    /// Curvas de nivel para las cotas dadas.
    pub fn curvas_nivel(&self, cotas: &[f64]) -> Vec<Curva> {
        let mut curvas = Vec::new();
        for &nivel in cotas {
            let arriba = |i: usize| self.xyz[i][2] >= nivel;
            let corta: Vec<bool> = self.aristas.iter().map(|e| arriba(e[0]) != arriba(e[1])).collect();
            if !corta.iter().any(|&c| c) {
                continue;
            }
            let punto = |k: usize| {
                let (a, b) = (self.xyz[self.aristas[k][0]], self.xyz[self.aristas[k][1]]);
                let t = (nivel - a[2]) / (b[2] - a[2]);
                [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
            };
            let mut pares = Vec::new();
            for at in &self.aristas_tri {
                let c: Vec<usize> = at.iter().copied().filter(|&k| corta[k]).collect();
                if c.len() == 2 {
                    pares.push((c[0], c[1]));
                }
            }
            for (cadena, cerrada) in encadenar(&pares) {
                let mut xy: Vec<[f64; 2]> = Vec::new();
                for k in cadena {
                    let p = punto(k);
                    if xy.last().map_or(true, |q: &[f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) > 1e-9) {
                        xy.push(p);
                    }
                }
                if xy.len() < 2 {
                    continue;
                }
                let cerrada = cerrada && xy.len() >= 3;
                let c = Curva { cota: nivel, xy, cerrada };
                if c.longitud() > 1e-6 {
                    curvas.push(c);
                }
            }
        }
        curvas
    }
}

/// Une pares de aristas (grafo de grado ≤ 2) en cadenas abiertas o cerradas.
fn encadenar(pares: &[(usize, usize)]) -> Vec<(Vec<usize>, bool)> {
    let mut adj: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut orden: Vec<usize> = Vec::new();
    for &(a, b) in pares {
        for (x, y) in [(a, b), (b, a)] {
            if !adj.contains_key(&x) {
                orden.push(x);
            }
            adj.entry(x).or_default().push(y);
        }
    }
    let mut visitado: HashSet<usize> = HashSet::new();
    let mut cadenas = Vec::new();
    let recorrer = |inicio: usize, visitado: &mut HashSet<usize>| {
        let mut cadena = vec![inicio];
        visitado.insert(inicio);
        let mut actual = inicio;
        loop {
            match adj[&actual].iter().find(|n| !visitado.contains(n)) {
                None => return cadena,
                Some(&s) => {
                    actual = s;
                    visitado.insert(s);
                    cadena.push(s);
                }
            }
        }
    };
    for &nodo in orden.iter().filter(|n| adj[n].len() == 1) {
        if !visitado.contains(&nodo) {
            cadenas.push((recorrer(nodo, &mut visitado), false));
        }
    }
    for &nodo in &orden {
        if !visitado.contains(&nodo) {
            cadenas.push((recorrer(nodo, &mut visitado), true));
        }
    }
    cadenas
}

/// Cotas múltiplo de la equidistancia en `[zmin, zmax]`.
pub fn niveles(zmin: f64, zmax: f64, equidistancia: f64) -> Result<Vec<f64>, String> {
    if equidistancia <= 0.0 {
        return Err("La equidistancia debe ser positiva".into());
    }
    let k0 = (zmin / equidistancia - 1e-9).ceil() as i64;
    let k1 = (zmax / equidistancia + 1e-9).floor() as i64;
    Ok((k0..=k1).map(|k| crate::numerico::redondear(k as f64 * equidistancia, 6)).collect())
}

/// True si la curva es "mayor" (cada `cada` equidistancias).
pub fn es_mayor(cota: f64, equidistancia: f64, cada: i64) -> bool {
    ((cota / equidistancia).round() as i64).rem_euclid(cada) == 0
}
