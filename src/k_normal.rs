use ordered_float::OrderedFloat;
use syntax::{IntBin, FloatBin, CompBin, Syntax, Type};
use id::{IdGen};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KNormal {
    Unit,
    Int(i64),
    Float(OrderedFloat<f64>),
    Neg(String),
    IntBin(IntBin, String, String),
    FNeg(String),
    FloatBin(FloatBin, String, String),
    IfComp(CompBin, String, String, Box<KNormal>, Box<KNormal>),
    Let((String, Type), Box<KNormal>, Box<KNormal>),
    Var(String),
    LetRec(KFundef, Box<KNormal>),
    App(String, Box<[String]>),
    Tuple(Box<[String]>),
    LetTuple(Box<[(String, Type)]>, String, Box<KNormal>),
    Get(String, String),
    Put(String, String, String),
    ExtArray(String),
    ExtFunApp(String, Box<[String]>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KFundef {
    pub name: (String, Type),
    pub args: Box<[(String, Type)]>,
    pub body: Box<KNormal>,
}

/*
 * Free variables in an AST
 */
fn fv(e: &KNormal) -> HashSet<String> {
    macro_rules! invoke {
        ($e:expr) => (fv($e));
    }
    macro_rules! build_set {
        ($($x:expr),*) => ({
            let mut h = HashSet::new();
            $(h.insert($x.clone());)*
            h
        })
    }
    match *e {
        KNormal::Unit => HashSet::new(),
        KNormal::Int(_) => HashSet::new(),
        KNormal::Float(_) => HashSet::new(),
        KNormal::ExtArray(_) => HashSet::new(),
        KNormal::Neg(ref x) => build_set!(x),
        KNormal::FNeg(ref x) => build_set!(x),
        KNormal::IntBin(_, ref x, ref y) => build_set!(x, y),
        KNormal::FloatBin(_, ref x, ref y) => build_set!(x, y),
        KNormal::IfComp(_, ref x, ref y, ref e1, ref e2) => {
            let h = build_set!(x, y);
            let s1 = invoke!(e1);
            let s2 = invoke!(e2);
            &(&h | &s1) | &s2
        },
        KNormal::Let((ref x, _), ref e1, ref e2) => {
            let s1 = invoke!(e1);
            let s2 = &invoke!(e2) - &build_set!(x);
            &s1 | &s2
        }
        KNormal::Var(ref x) => build_set!(x),
        KNormal::LetRec(ref fundef, ref e2) => {
            let yts = &fundef.args;
            let e1 = &fundef.body;
            let (ref x, _) = fundef.name;
            let zs = &invoke!(e1) - &yts.iter().map(|x| x.0.clone()).collect();
            &(&zs | &invoke!(e2)) - &build_set!(x)
        },
        KNormal::App(ref x, ref ys) =>
            &build_set!(x) | &ys.iter().cloned().collect::<HashSet<_>>(),
        KNormal::Tuple(ref xs) => xs.iter().cloned().collect(),
        KNormal::LetTuple(ref xs, ref y, ref e) => {
            let tmp: HashSet<String> = xs.iter().map(|x| x.0.clone())
                .collect(); // S.of_list (List.map fst xs)
            &build_set!(y) | &(&invoke!(e) - &tmp)
        },
        KNormal::Get(ref x, ref y) => build_set!(x, y),
        KNormal::Put(ref x, ref y, ref z) => build_set!(x, y, z),
        KNormal::ExtFunApp(_, ref xs) => xs.iter().cloned().collect(),
    }
}


fn g(env: &HashMap<String, Type>, e: Syntax, id_gen: &mut IdGen, extenv: &HashMap<String, Type>)
     -> (KNormal, Type) {
    /*
     * Function insert_let is moved here, and realized as a macro.
     */
    macro_rules! insert_let_macro {
        ($et:expr, $k:expr) => ({
            let (e, t) = $et;
            match e {
                KNormal::Var(x) => $k(x),
                _ => {
                    let x = id_gen.gen_tmp(&t);
                    let (e2, t2) = $k(x.clone());
                    (KNormal::Let((x, t), Box::new(e), Box::new(e2)), t2)
                },
            }
        });
    }
    macro_rules! insert_let_macro_with_env {
        ($et:expr, $k:expr, $env:expr, $id_gen:expr, $extenv:expr) => ({
            let (e, t) = $et;
            match e {
                KNormal::Var(x) => $k(x),
                _ => {
                    let x = $id_gen.gen_tmp(&t);
                    let (e2, t2) = $k(x.clone());
                    (KNormal::Let((x, t), Box::new(e), Box::new(e2)), t2)
                },
            }
        });
    }
    macro_rules! invoke {
        ($e:expr) => (g(env, $e, id_gen, extenv));
    }
    // (Syntax, F) -> KNormal where F: Fn(String) -> (KNormal, Type)
    macro_rules! insert_let_helper {
        ($e:expr, $k:expr) => ({
            let e = invoke!($e);
            insert_let_macro!(e, $k)
        });
    }
    macro_rules! insert_let_helper_with_env {
        ($e:expr, $k:expr, $env:expr, $id_gen:expr, $extenv:expr) => ({
            let e = g($env, $e, $id_gen, $extenv);
            insert_let_macro_with_env!(e, $k, $env, $id_gen, $extenv)
        });
    }
    macro_rules! insert_let_binop {
        ($e1:expr, $e2:expr, $constr:expr, $ty:expr, $op:expr) => (
            insert_let_helper!(
                $e1, |x| insert_let_helper!(
                    ($e2).clone(),
                    |y| ($constr($op, x, y), $ty))));
    }

    match e {
        Syntax::Unit => (KNormal::Unit, Type::Unit),
        // bool is converted to int here
        Syntax::Bool(b) => (KNormal::Int(if b { 1 } else { 0 }), Type::Int),
        Syntax::Int(i) => (KNormal::Int(i), Type::Int),
        Syntax::Float(d) => (KNormal::Float(d), Type::Float),
        Syntax::Not(e) =>
            invoke!(Syntax::If(e,
                               Box::new(Syntax::Bool(false)),
                               Box::new(Syntax::Bool(true)))),
        Syntax::Neg(e) =>
            insert_let_helper!(*e, |x| (KNormal::Neg(x), Type::Int)),
        Syntax::IntBin(op, e1, e2) =>
            insert_let_binop!(*e1, *e2, KNormal::IntBin, Type::Int, op),
        Syntax::FNeg(e) =>
            insert_let_helper!(*e, |x| (KNormal::FNeg(x), Type::Float)),
        Syntax::FloatBin(op, e1, e2) =>
            insert_let_binop!(*e1, *e2, KNormal::FloatBin, Type::Float, op),
        Syntax::CompBin(_, _, _) =>
            invoke!(Syntax::If(Box::new(e),
                               Box::new(Syntax::Bool(true)),
                               Box::new(Syntax::Bool(false)))),
        Syntax::If(e1, e3, e4) => {
            let tmp = *e1; // http://stackoverflow.com/questions/28466809/collaterally-moved-error-on-deconstructing-box-of-pairs
            match tmp {
                Syntax::Not(e1) => invoke!(Syntax::If(e1, e4, e3)),
                Syntax::CompBin(op, e1, e2) =>
                    insert_let_helper!(
                        *e1,
                        |x| insert_let_helper!(
                            (*e2).clone(), // TODO maybe unnecessary cloning, but needed to squelch borrow checker
                            |y| {
                                let (e3p, t3) = invoke!((*e3).clone());
                                let (e4p, t4) = invoke!((*e4).clone());
                                (KNormal::IfComp(op, x, y, Box::new(e3p),
                                                 Box::new(e4p)), t3)
                            })),
                _ => invoke!(Syntax::If(Box::new(Syntax::CompBin(CompBin::Eq, Box::new(tmp), Box::new(Syntax::Bool(false)))),
                                        e4, e3)),
            }
        },
        Syntax::Let((x, t), e1, e2) => {
            let (e1, t1) = invoke!(*e1);
            let mut cp_env = env.clone();
            cp_env.insert(x.clone(), t.clone());
            let (e2, t2) = g(&cp_env, *e2, id_gen, extenv);
            (KNormal::Let((x, t), Box::new(e1), Box::new(e2)), t2)
        },
        Syntax::Var(x) => {
            if let Some(t) = env.get(&x).cloned() {
                return (KNormal::Var(x), t);
            }
            let ty = extenv.get(&x).unwrap().clone(); // It is guaranteed that there is.
            match ty {
                Type::Array(_) => (KNormal::ExtArray(x), ty),
                _ => panic!(format!("external variable {} does not have an array type", x)),
            }
        },
        Syntax::LetRec(_, e2) => unimplemented!(),
        Syntax::App(e1, e2s) => {
            let g_e1 = invoke!(*e1);
            match g_e1.1.clone() {
                Type::Fun(_, t) => {
                    // TODO very rough way to simulate an internal recursive function in the original code
                    fn bind(mut xs: Vec<String>, p: usize, id_gen: &mut IdGen, extenv: &HashMap<String, Type>, env: &HashMap<String, Type>, e2s: &[Syntax], t: Type, f: String) -> (KNormal, Type) {
                        let n = e2s.len();
                        if p == n {
                            (KNormal::App(
                                f.clone(),
                                xs.clone().into_boxed_slice()),
                             t.clone())
                        } else {
                            insert_let_helper_with_env!(e2s[p].clone(),
                                                        |x: String| {
                                                            xs.push(x.clone());
                                                            bind(xs, p + 1, id_gen, extenv, env, e2s, t, f)
                                                        }, env, id_gen, extenv)
                        }
                    }
                    insert_let_macro!(
                        g_e1.clone(),
                        |f: String|
                        bind(Vec::new(), 0, id_gen, extenv, env,
                             &e2s, (*t).clone(), f))
                },
                _ => panic!(),
            }
        },
        Syntax::Tuple(es) => {
            fn bind(mut xs: Vec<String>, mut ts: Vec<Type>, p: usize,
                    env: &HashMap<String, Type>, id_gen: &mut IdGen,
                    extenv: &HashMap<String, Type>, es: &[Syntax])
                    -> (KNormal, Type) {
                let n = es.len();
                if p == n {
                    (KNormal::Tuple(xs.into_boxed_slice()),
                     Type::Tuple(ts.into_boxed_slice()))
                } else {
                    let (ex, tx) = g(env, es[p].clone(), id_gen, extenv); 
                    insert_let_macro_with_env!((ex, tx.clone()),
                                                |x: String| {
                                                    xs.push(x.clone());
                                                    ts.push(tx);
                                                    bind(xs, ts, p + 1,
                                                         env, id_gen, extenv,
                                                         es)
                                                }, env, id_gen, extenv)
                }
            }
            bind(Vec::new(), Vec::new(), 0, env, id_gen, extenv, &es)
        },
        Syntax::LetTuple(xts, e1, e2) => {
            insert_let_helper!(*e1,
                               move |y| {
                                   let mut cp_env = env.clone();
                                   for &(ref x, ref t) in xts.iter() {
                                       cp_env.insert(x.clone(), t.clone());
                                   }
                                   let (e2p, t2) = g(&cp_env, *e2, id_gen,
                                                     extenv);
                                   (KNormal::LetTuple(xts, y, Box::new(e2p)),
                                    t2)
                               }
            )
        },
        Syntax::Array(e1, e2) => unimplemented!(),
        Syntax::Get(e1, e2) => {
            let g_e1 = invoke!(*e1);
            match g_e1.1.clone() {
                Type::Array(ref t) => insert_let_macro!(
                    g_e1,
                    |x| insert_let_helper!(
                        (*e2).clone(), |y| (KNormal::Get(x, y), (t as &Type).clone()))),
                _ => panic!("e1 should be an array")
            }
        },
        Syntax::Put(e1, e2, e3) => {
            insert_let_helper!(
                *e1, |x| insert_let_helper!(
                    (*e2).clone(), |y| insert_let_helper!(
                        (*e3).clone(), |z|
                        (KNormal::Put(x, y, z), Type::Unit))))
                
        },
    } // TODO 2 cases remaining
}



#[cfg(test)]
mod tests {
    use syntax::*;
    use k_normal::*;
    #[test]
    fn test_fv_let() {
        use self::KNormal::*;
        // let x: int = y + y in z - x, fv(...) = {y, z}
        let x = || "x".to_string();
        let y = || "y".to_string();
        let z = || "z".to_string();
        let expr = Let((x(), Type::Int),
                       Box::new(IntBin(::syntax::IntBin::Add, y(), y())),
                       Box::new(IntBin(::syntax::IntBin::Sub, z(), x())));
        assert_eq!(fv(&expr), vec![y(), z()].into_iter().collect());
    }
    #[test]
    fn test_fv_let_unused() {
        use self::KNormal::*;
        // let x: int = y + y in z, fv(...) = {y, z}
        let x = || "x".to_string();
        let y = || "y".to_string();
        let z = || "z".to_string();
        let expr = Let((x(), Type::Int),
                       Box::new(IntBin(::syntax::IntBin::Add, y(), y())),
                       Box::new(Var(z())));
        assert_eq!(fv(&expr), vec![y(), z()].into_iter().collect());
    }
    #[test]
    fn test_g_if() {
        let mut id_gen = IdGen::new();
        let extenv = HashMap::new();
        // if x = x then y else z
        // ==> IfComp(Eq, x, x, y, z)
        let x = || "x".to_string();
        let y = || "y".to_string();
        let z = || "z".to_string();
        let env = vec![(x(), Type::Int),
                       (y(), Type::Int),
                       (z(), Type::Int)].into_iter().collect();
        let expr = Syntax::If(Box::new(Syntax::CompBin(CompBin::Eq, Box::new(Syntax::Var(x())), Box::new(Syntax::Var(x())))),
                              Box::new(Syntax::Var(y())),
                              Box::new(Syntax::Var(z())));
        assert_eq!(g(&env, expr, &mut id_gen, &extenv),
                   (KNormal::IfComp(CompBin::Eq,
                                    x(), x(),
                                    Box::new(KNormal::Var(y())),
                                    Box::new(KNormal::Var(z()))), Type::Int))
    }
    #[test]
    fn test_g_app() {
        let mut id_gen = IdGen::new();
        let extenv = HashMap::new();
        // x y z
        // ==> App(x, [y, z])
        let x = || "x".to_string();
        let y = || "y".to_string();
        let z = || "z".to_string();
        let env = vec![(x(), Type::Fun(vec![Type::Int; 2].into_boxed_slice(),
                                       Box::new(Type::Int))),
                       (y(), Type::Int),
                       (z(), Type::Int)].into_iter().collect();
        let expr = Syntax::App(Box::new(Syntax::Var(x())),
                               vec![Syntax::Var(y()), Syntax::Var(z())].into_boxed_slice());
        assert_eq!(g(&env, expr, &mut id_gen, &extenv),
                   (KNormal::App(x(), vec![y(), z()].into_boxed_slice()),
                    Type::Int))
    }
    #[test]
    fn test_g_tuple() {
        let mut id_gen = IdGen::new();
        let extenv = HashMap::new();
        // (x, y, 1.0)
        // ==> Let(d0, 1.0, Tuple(x, y, d0))
        let x = || "x".to_string();
        let y = || "y".to_string();
        let d0 = || "d0".to_string(); // Temporary variable of type float
        let env = vec![(x(), Type::Bool),
                       (y(), Type::Int)].into_iter().collect();
        let expr = Syntax::Tuple(vec![Syntax::Var(x()),
                                 Syntax::Var(y()),
                                 Syntax::Float(1.0.into())].into_boxed_slice());
        assert_eq!(g(&env, expr, &mut id_gen, &extenv),
                   (KNormal::Let((d0(), Type::Float),
                                 Box::new(KNormal::Float(1.0.into())),
                                 Box::new(KNormal::Tuple(
                                     vec![x(), y(), d0()].into_boxed_slice()))),
                    Type::Tuple(vec![Type::Bool, Type::Int, Type::Float]
                                .into_boxed_slice())))
    }
    #[test]
    fn test_g_let_tuple() {
        let mut id_gen = IdGen::new();
        let extenv = HashMap::new();
        // let (x: bool, y: int, z: float) = f(1) in x
        // ==> LetTuple([x, y, z], f(1), x)
        // f: int -> (bool, int, float)
        let x = || "x".to_string();
        let y = || "y".to_string();
        let z = || "z".to_string();
        let f = || "f".to_string();
        let i0 = || "i0".to_string(); // temporary variable of type int
        let t1 = || "t1".to_string(); // temporary variable of type tuple
        let tuple_ty = Type::Tuple(vec![Type::Bool, Type::Int, Type::Float]
                                   .into_boxed_slice());
        let env = vec![(f(),
                        Type::Fun(vec![Type::Int].into_boxed_slice(),
                                  Box::new(tuple_ty.clone())))]
            .into_iter().collect();
        let args_list = vec![(x(), Type::Bool),
                             (y(), Type::Int),
                             (z(), Type::Float)]
            .into_boxed_slice();
        let expr = Syntax::LetTuple(
            args_list.clone(),
            Box::new(Syntax::App(Box::new(Syntax::Var(f())),
                                 vec![Syntax::Int(1)].into_boxed_slice())),
            Box::new(Syntax::Var(x())));
        let expected = (KNormal::Let((t1(), tuple_ty),
                                     Box::new(
                                         KNormal::Let(
                                             (i0(), Type::Int),
                                             Box::new(KNormal::Int(1)),
                                             Box::new(KNormal::App(
                                                 f(),
                                                 vec![i0()]
                                                     .into_boxed_slice())))),
                                     Box::new(
                                         KNormal::LetTuple(
                                             args_list,
                                             t1(),
                                             Box::new(KNormal::Var(x()))))),
        Type::Bool);
        assert_eq!(g(&env, expr, &mut id_gen, &extenv),
                   expected);
    }
}