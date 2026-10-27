//! UI strings in Spanish and English, keyed like `cmd.solid.box`.
//!
//! Command labels come from the registry (`CommandSpec::label`); every other UI
//! string uses a `ui.` key. Missing Spanish strings fall back to English, and a
//! missing English string shows the key itself, so a gap is visible, never a crash.

/// A UI language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lang {
    /// English (fallback).
    #[default]
    En,
    /// Spanish.
    Es,
}

impl Lang {
    /// From a BCP 47 tag such as `es-MX` or `en_US.UTF-8`; unknown tags give English.
    pub fn from_tag(tag: &str) -> Self {
        let primary = tag
            .split(['-', '_', '.'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match primary.as_str() {
            "es" => Self::Es,
            _ => Self::En,
        }
    }

    /// The system language, or English if it cannot be read.
    pub fn system() -> Self {
        sys_locale::get_locale()
            .map(|t| Self::from_tag(&t))
            .unwrap_or_default()
    }
}

/// String lookup for one language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct I18n {
    /// Current language.
    pub lang: Lang,
}

impl I18n {
    /// Lookup for `lang`.
    pub const fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// The string for `key`, falling back to English and then to the key.
    pub fn tr<'a>(&self, key: &'a str) -> &'a str {
        let table = match self.lang {
            Lang::En => EN,
            Lang::Es => ES,
        };
        find(table, key).or_else(|| find(EN, key)).unwrap_or(key)
    }
}

fn find(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// English strings.
pub const EN: &[(&str, &str)] = &[
    // Commands (labels from the registry).
    ("cmd.solid.box", "Box"),
    ("cmd.face.push_pull", "Push/Pull"),
    ("cmd.solid.cylinder", "Cylinder"),
    ("cmd.sketch.create", "Sketch"),
    ("cmd.sketch.add", "Add to sketch"),
    ("cmd.sketch.set_dimension", "Edit dimension"),
    ("cmd.sketch.delete", "Delete from sketch"),
    ("cmd.sketch.move_point", "Move point"),
    ("cmd.solid.extrude", "Extrude"),
    ("cmd.edge.fillet", "Fillet"),
    ("cmd.edge.chamfer", "Chamfer"),
    ("cmd.body.shell", "Shell"),
    ("cmd.body.boolean", "Combine"),
    ("cmd.edit.undo", "Undo"),
    ("cmd.edit.redo", "Redo"),
    ("cmd.selection.set", "Select"),
    ("cmd.doc.new", "New"),
    ("cmd.doc.open", "Open…"),
    ("cmd.doc.save", "Save…"),
    ("cmd.file.export_step", "Export STEP…"),
    ("cmd.file.export_stl", "Export STL…"),
    ("cmd.doc.inspect", "Inspect"),
    ("cmd.body.faces", "List faces"),
    ("cmd.body.measure", "Measure"),
    // Toolbar groups.
    ("ui.group.create", "Create"),
    ("ui.group.modify", "Modify"),
    ("ui.group.edit", "Edit"),
    // Menus and panels.
    ("ui.menu.file", "File"),
    ("ui.menu.view", "View"),
    ("ui.tab.viewport", "Viewport"),
    ("ui.tab.features", "Features"),
    (
        "ui.features.empty",
        "No features yet. Create a box to start.",
    ),
    ("ui.features.selection", "Selection"),
    ("ui.features.none_selected", "Nothing selected"),
    ("ui.features.kernel", "Kernel"),
    ("ui.status.ready", "Ready"),
    ("ui.status.hover", "Face"),
    ("ui.status.selected_faces", "Faces"),
    ("ui.status.selected_edges", "Edges"),
    ("ui.status.hover_edge", "Edge"),
    ("ui.status.failed", "Could not run"),
    ("ui.done.solid.box", "Box created"),
    ("ui.done.face.push_pull", "Face moved"),
    ("ui.done.edit.undo", "Undone"),
    ("ui.done.edit.redo", "Redone"),
    ("ui.done.doc.new", "New document"),
    ("ui.done.doc.open", "Document opened"),
    ("ui.done.doc.save", "Document saved"),
    ("ui.done.file.export_step", "STEP exported"),
    ("ui.done.file.export_stl", "STL exported"),
    ("ui.status.no_gpu", "No GPU: the viewport is unavailable"),
    // Views.
    ("ui.view.fit", "Fit (F)"),
    ("ui.view.iso", "Iso"),
    ("ui.view.top", "Top"),
    ("ui.view.front", "Front"),
    ("ui.view.right", "Right"),
    ("ui.view.theme_dark", "Dark theme"),
    ("ui.view.theme_light", "Light theme"),
    // Parameter popovers.
    ("ui.param.size", "Size (mm)"),
    ("ui.param.origin", "Origin (mm)"),
    ("ui.param.face", "Face"),
    ("ui.param.distance", "Distance (mm)"),
    ("ui.param.need_face", "Select one face first."),
    ("ui.param.need_edge", "Select at least one edge first."),
    (
        "ui.param.need_region",
        "Select a closed sketch region first.",
    ),
    ("ui.sketch.pick_plane", "Sketch plane"),
    (
        "ui.sketch.plane_hint",
        "Pick a plane, or select a planar face first.",
    ),
    ("ui.sketch.select", "Select"),
    ("ui.sketch.line", "Line"),
    ("ui.sketch.rectangle", "Rectangle"),
    ("ui.sketch.circle", "Circle"),
    ("ui.sketch.arc", "Arc"),
    ("ui.sketch.dimension", "Dimension"),
    ("ui.sketch.construction", "Construction"),
    ("ui.sketch.finish", "Finish sketch"),
    ("ui.sketch.dof", "Degrees of freedom"),
    ("ui.sketch.fully", "Fully constrained"),
    ("ui.sketch.conflict", "Conflicting constraints"),
    ("ui.sketch.remove", "Remove"),
    ("ui.sketch.redundant", "Redundant"),
    ("ui.param.radius", "Radius (mm)"),
    ("ui.param.height", "Height (mm)"),
    ("ui.param.thickness", "Thickness (mm)"),
    ("ui.param.target", "Body"),
    ("ui.param.tool", "Tool body"),
    ("ui.bool.union", "Union"),
    ("ui.bool.subtract", "Subtract"),
    ("ui.bool.intersect", "Intersect"),
    ("ui.value.invalid", "Not a valid length"),
    ("ui.value.not_length", "That is not a length"),
    (
        "ui.value.hint",
        "Enter to apply · Esc to cancel · e.g. 5, 3/4 in, 2 cm",
    ),
    ("ui.apply", "Apply"),
    ("ui.cancel", "Cancel"),
    // Feature status.
    ("ui.feature.ok", "OK"),
    ("ui.feature.failed", "Failed"),
    ("ui.feature.suppressed", "Suppressed"),
];

/// Spanish strings.
pub const ES: &[(&str, &str)] = &[
    ("cmd.solid.box", "Caja"),
    ("cmd.face.push_pull", "Empujar/Jalar"),
    ("cmd.solid.cylinder", "Cilindro"),
    ("cmd.sketch.create", "Croquis"),
    ("cmd.sketch.add", "Agregar al croquis"),
    ("cmd.sketch.set_dimension", "Editar cota"),
    ("cmd.sketch.delete", "Borrar del croquis"),
    ("cmd.sketch.move_point", "Mover punto"),
    ("cmd.solid.extrude", "Extruir"),
    ("cmd.edge.fillet", "Redondeo"),
    ("cmd.edge.chamfer", "Chaflán"),
    ("cmd.body.shell", "Vaciado"),
    ("cmd.body.boolean", "Combinar"),
    ("cmd.edit.undo", "Deshacer"),
    ("cmd.edit.redo", "Rehacer"),
    ("cmd.selection.set", "Seleccionar"),
    ("cmd.doc.new", "Nuevo"),
    ("cmd.doc.open", "Abrir…"),
    ("cmd.doc.save", "Guardar…"),
    ("cmd.file.export_step", "Exportar STEP…"),
    ("cmd.file.export_stl", "Exportar STL…"),
    ("cmd.doc.inspect", "Inspeccionar"),
    ("cmd.body.faces", "Listar caras"),
    ("cmd.body.measure", "Medir"),
    ("ui.group.create", "Crear"),
    ("ui.group.modify", "Modificar"),
    ("ui.group.edit", "Editar"),
    ("ui.menu.file", "Archivo"),
    ("ui.menu.view", "Vista"),
    ("ui.tab.viewport", "Vista 3D"),
    ("ui.tab.features", "Operaciones"),
    (
        "ui.features.empty",
        "Aún no hay operaciones. Crea una caja para empezar.",
    ),
    ("ui.features.selection", "Selección"),
    ("ui.features.none_selected", "Nada seleccionado"),
    ("ui.features.kernel", "Kernel"),
    ("ui.status.ready", "Listo"),
    ("ui.status.hover", "Cara"),
    ("ui.status.selected_faces", "Caras"),
    ("ui.status.selected_edges", "Aristas"),
    ("ui.status.hover_edge", "Arista"),
    ("ui.status.failed", "No se pudo ejecutar"),
    ("ui.done.solid.box", "Caja creada"),
    ("ui.done.face.push_pull", "Cara desplazada"),
    ("ui.done.edit.undo", "Deshecho"),
    ("ui.done.edit.redo", "Rehecho"),
    ("ui.done.doc.new", "Documento nuevo"),
    ("ui.done.doc.open", "Documento abierto"),
    ("ui.done.doc.save", "Documento guardado"),
    ("ui.done.file.export_step", "STEP exportado"),
    ("ui.done.file.export_stl", "STL exportado"),
    (
        "ui.status.no_gpu",
        "Sin GPU: la vista 3D no está disponible",
    ),
    ("ui.view.fit", "Encuadrar (F)"),
    ("ui.view.iso", "Iso"),
    ("ui.view.top", "Superior"),
    ("ui.view.front", "Frontal"),
    ("ui.view.right", "Derecha"),
    ("ui.view.theme_dark", "Tema oscuro"),
    ("ui.view.theme_light", "Tema claro"),
    ("ui.param.size", "Tamaño (mm)"),
    ("ui.param.origin", "Origen (mm)"),
    ("ui.param.face", "Cara"),
    ("ui.param.distance", "Distancia (mm)"),
    ("ui.param.need_face", "Selecciona primero una cara."),
    (
        "ui.param.need_edge",
        "Selecciona primero al menos una arista.",
    ),
    (
        "ui.param.need_region",
        "Selecciona primero una región cerrada del croquis.",
    ),
    ("ui.sketch.pick_plane", "Plano del croquis"),
    (
        "ui.sketch.plane_hint",
        "Elige un plano, o selecciona antes una cara plana.",
    ),
    ("ui.sketch.select", "Seleccionar"),
    ("ui.sketch.line", "Línea"),
    ("ui.sketch.rectangle", "Rectángulo"),
    ("ui.sketch.circle", "Círculo"),
    ("ui.sketch.arc", "Arco"),
    ("ui.sketch.dimension", "Cota"),
    ("ui.sketch.construction", "Construcción"),
    ("ui.sketch.finish", "Terminar croquis"),
    ("ui.sketch.dof", "Grados de libertad"),
    ("ui.sketch.fully", "Totalmente restringido"),
    ("ui.sketch.conflict", "Restricciones en conflicto"),
    ("ui.sketch.remove", "Quitar"),
    ("ui.sketch.redundant", "Redundantes"),
    ("ui.param.radius", "Radio (mm)"),
    ("ui.param.height", "Altura (mm)"),
    ("ui.param.thickness", "Espesor (mm)"),
    ("ui.param.target", "Cuerpo"),
    ("ui.param.tool", "Cuerpo herramienta"),
    ("ui.bool.union", "Unir"),
    ("ui.bool.subtract", "Restar"),
    ("ui.bool.intersect", "Intersecar"),
    ("ui.value.invalid", "No es una longitud válida"),
    ("ui.value.not_length", "Eso no es una longitud"),
    (
        "ui.value.hint",
        "Enter aplica · Esc cancela · ej. 5, 3/4 in, 2 cm",
    ),
    ("ui.apply", "Aplicar"),
    ("ui.cancel", "Cancelar"),
    ("ui.feature.ok", "OK"),
    ("ui.feature.failed", "Falló"),
    ("ui.feature.suppressed", "Suprimida"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn tags_map_to_languages() {
        assert_eq!(Lang::from_tag("es-MX"), Lang::Es);
        assert_eq!(Lang::from_tag("es_ES.UTF-8"), Lang::Es);
        assert_eq!(Lang::from_tag("en-US"), Lang::En);
        assert_eq!(Lang::from_tag("fr-FR"), Lang::En);
        assert_eq!(Lang::from_tag(""), Lang::En);
    }

    #[test]
    fn lookup_falls_back_to_english_then_key() {
        let es = I18n::new(Lang::Es);
        assert_eq!(es.tr("cmd.solid.box"), "Caja");
        assert_eq!(I18n::new(Lang::En).tr("cmd.solid.box"), "Box");
        assert_eq!(es.tr("ui.missing.key"), "ui.missing.key");
    }

    #[test]
    fn both_tables_have_the_same_keys_once() {
        let keys = |t: &'static [(&'static str, &'static str)]| {
            let set: BTreeSet<&str> = t.iter().map(|(k, _)| *k).collect();
            assert_eq!(set.len(), t.len(), "duplicate key");
            set
        };
        assert_eq!(keys(EN), keys(ES));
    }

    #[test]
    fn every_registry_label_is_translated() {
        let registry = kubit_engine::Session::new(Box::new(|| {
            Box::new(kubit_kernel_api::testing::BoxKernel::new())
        }))
        .registry()
        .clone();
        for spec in registry.iter() {
            for table in [EN, ES] {
                assert!(
                    find(table, spec.label).is_some(),
                    "{} untranslated",
                    spec.label
                );
            }
            if let Some(group) = spec.toolbar_group {
                let key = format!("ui.group.{group}");
                assert!(find(EN, &key).is_some(), "{key} untranslated");
            }
        }
    }
}
