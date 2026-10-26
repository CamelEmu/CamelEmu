// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint, type=warning, deprecated_member_use, deprecated_member_use_from_same_package
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'capture.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$CaptureEvent {

 Object get field0;



@override
bool operator ==(Object other) {
  final _this = this as CaptureEvent;
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureEvent&&const DeepCollectionEquality().equals(other.field0, _this.field0));
}


@override
int get hashCode {
  final _this = this as CaptureEvent;
  return Object.hash(runtimeType,const DeepCollectionEquality().hash(_this.field0));
}

@override
String toString() {
  final _this = this as CaptureEvent;
  return 'CaptureEvent(field0: ${_this.field0})';
}


}

/// @nodoc
class $CaptureEventCopyWith<$Res>  {
$CaptureEventCopyWith(CaptureEvent _, $Res Function(CaptureEvent) __);
}


/// Adds pattern-matching-related methods to [CaptureEvent].
extension CaptureEventPatterns on CaptureEvent {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CaptureEvent_Progress value)?  progress,TResult Function( CaptureEvent_Finished value)?  finished,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CaptureEvent_Progress() when progress != null:
return progress(_that);case CaptureEvent_Finished() when finished != null:
return finished(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CaptureEvent_Progress value)  progress,required TResult Function( CaptureEvent_Finished value)  finished,}){
final _that = this;
switch (_that) {
case CaptureEvent_Progress():
return progress(_that);case CaptureEvent_Finished():
return finished(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CaptureEvent_Progress value)?  progress,TResult? Function( CaptureEvent_Finished value)?  finished,}){
final _that = this;
switch (_that) {
case CaptureEvent_Progress() when progress != null:
return progress(_that);case CaptureEvent_Finished() when finished != null:
return finished(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( CaptureProgress field0)?  progress,TResult Function( CaptureResult field0)?  finished,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CaptureEvent_Progress() when progress != null:
return progress(_that.field0);case CaptureEvent_Finished() when finished != null:
return finished(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( CaptureProgress field0)  progress,required TResult Function( CaptureResult field0)  finished,}) {final _that = this;
switch (_that) {
case CaptureEvent_Progress():
return progress(_that.field0);case CaptureEvent_Finished():
return finished(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( CaptureProgress field0)?  progress,TResult? Function( CaptureResult field0)?  finished,}) {final _that = this;
switch (_that) {
case CaptureEvent_Progress() when progress != null:
return progress(_that.field0);case CaptureEvent_Finished() when finished != null:
return finished(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class CaptureEvent_Progress extends CaptureEvent {
  const CaptureEvent_Progress(this.field0): super._();
  

@override final  CaptureProgress field0;

/// Create a copy of CaptureEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CaptureEvent_ProgressCopyWith<CaptureEvent_Progress> get copyWith => _$CaptureEvent_ProgressCopyWithImpl<CaptureEvent_Progress>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureEvent_Progress&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,field0);
}

@override
String toString() {
    return 'CaptureEvent.progress(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $CaptureEvent_ProgressCopyWith<$Res> implements $CaptureEventCopyWith<$Res> {
  factory $CaptureEvent_ProgressCopyWith(CaptureEvent_Progress value, $Res Function(CaptureEvent_Progress) _then) = _$CaptureEvent_ProgressCopyWithImpl;
@useResult
$Res call({
 CaptureProgress field0
});




}
/// @nodoc
class _$CaptureEvent_ProgressCopyWithImpl<$Res>
    implements $CaptureEvent_ProgressCopyWith<$Res> {
  _$CaptureEvent_ProgressCopyWithImpl(this._self, this._then);

  final CaptureEvent_Progress _self;
  final $Res Function(CaptureEvent_Progress) _then;

/// Create a copy of CaptureEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(CaptureEvent_Progress(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as CaptureProgress,
  ));
}


}

/// @nodoc


class CaptureEvent_Finished extends CaptureEvent {
  const CaptureEvent_Finished(this.field0): super._();
  

@override final  CaptureResult field0;

/// Create a copy of CaptureEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CaptureEvent_FinishedCopyWith<CaptureEvent_Finished> get copyWith => _$CaptureEvent_FinishedCopyWithImpl<CaptureEvent_Finished>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureEvent_Finished&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,field0);
}

@override
String toString() {
    return 'CaptureEvent.finished(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $CaptureEvent_FinishedCopyWith<$Res> implements $CaptureEventCopyWith<$Res> {
  factory $CaptureEvent_FinishedCopyWith(CaptureEvent_Finished value, $Res Function(CaptureEvent_Finished) _then) = _$CaptureEvent_FinishedCopyWithImpl;
@useResult
$Res call({
 CaptureResult field0
});




}
/// @nodoc
class _$CaptureEvent_FinishedCopyWithImpl<$Res>
    implements $CaptureEvent_FinishedCopyWith<$Res> {
  _$CaptureEvent_FinishedCopyWithImpl(this._self, this._then);

  final CaptureEvent_Finished _self;
  final $Res Function(CaptureEvent_Finished) _then;

/// Create a copy of CaptureEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(CaptureEvent_Finished(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as CaptureResult,
  ));
}


}

// dart format on
