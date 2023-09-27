use crate::{
    core::{algebra::Vector2, curve::Curve},
    curve::{CurveEditorBuilder, CurveEditorMessage},
    inspector::{
        editors::{
            PropertyEditorBuildContext, PropertyEditorDefinition, PropertyEditorInstance,
            PropertyEditorMessageContext, PropertyEditorTranslationContext,
        },
        FieldKind, InspectorError, PropertyChanged,
    },
    message::{MessageDirection, UiMessage},
    widget::WidgetBuilder,
    Thickness,
};
use std::any::TypeId;

#[derive(Debug)]
pub struct CurvePropertyEditorDefinition;

impl PropertyEditorDefinition for CurvePropertyEditorDefinition {
    fn value_type_id(&self) -> TypeId {
        TypeId::of::<Curve>()
    }

    fn create_instance(
        &self,
        ctx: PropertyEditorBuildContext,
    ) -> Result<PropertyEditorInstance, InspectorError> {
        let value = ctx.property_info.cast_value::<Curve>()?;
        Ok(PropertyEditorInstance::Simple {
            editor: CurveEditorBuilder::new(
                WidgetBuilder::new()
                    .with_min_size(Vector2::new(0.0, 200.0))
                    .with_margin(Thickness::uniform(1.0)),
            )
            .with_curve(value.clone())
            .build(ctx.build_context),
        })
    }

    fn create_message(
        &self,
        ctx: PropertyEditorMessageContext,
    ) -> Result<Option<UiMessage>, InspectorError> {
        let value = ctx.property_info.cast_value::<Curve>()?;
        Ok(Some(CurveEditorMessage::sync(
            ctx.instance,
            MessageDirection::ToWidget,
            value.clone(),
        )))
    }

    fn translate_message(&self, ctx: PropertyEditorTranslationContext) -> Option<PropertyChanged> {
        if ctx.message.direction() == MessageDirection::FromWidget {
            if let Some(CurveEditorMessage::Sync(value)) = ctx.message.data() {
                return Some(PropertyChanged {
                    name: ctx.name.to_string(),
                    owner_type_id: ctx.owner_type_id,
                    value: FieldKind::object(value.clone()),
                });
            }
        }
        None
    }
}
