# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: user_service.proto
"""Generated protocol buffer code."""
from google.protobuf.internal import builder as _builder
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import symbol_database as _symbol_database
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


import common_pb2 as common__pb2


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'\n\x12user_service.proto\x12\x12\x62lockjoy.api.ui_v1\x1a\x0c\x63ommon.proto\"?\n\x0eGetUserRequest\x12-\n\x04meta\x18\x01 \x01(\x0b\x32\x1f.blockjoy.api.ui_v1.RequestMeta\"i\n\x0fGetUserResponse\x12.\n\x04meta\x18\x01 \x01(\x0b\x32 .blockjoy.api.ui_v1.ResponseMeta\x12&\n\x04user\x18\x02 \x01(\x0b\x32\x18.blockjoy.api.ui_v1.User\"j\n\x11\x43reateUserRequest\x12-\n\x04meta\x18\x01 \x01(\x0b\x32\x1f.blockjoy.api.ui_v1.RequestMeta\x12&\n\x04user\x18\x02 \x01(\x0b\x32\x18.blockjoy.api.ui_v1.User\"D\n\x12\x43reateUserResponse\x12.\n\x04meta\x18\x01 \x01(\x0b\x32 .blockjoy.api.ui_v1.ResponseMeta\"\x8b\x01\n\x1aUpsertConfigurationRequest\x12-\n\x04meta\x18\x01 \x01(\x0b\x32\x1f.blockjoy.api.ui_v1.RequestMeta\x12>\n\x06params\x18\x02 \x03(\x0b\x32..blockjoy.api.ui_v1.UserConfigurationParameter\"M\n\x1bUpsertConfigurationResponse\x12.\n\x04meta\x18\x01 \x01(\x0b\x32 .blockjoy.api.ui_v1.ResponseMeta\"H\n\x17GetConfigurationRequest\x12-\n\x04meta\x18\x01 \x01(\x0b\x32\x1f.blockjoy.api.ui_v1.RequestMeta\"\x8a\x01\n\x18GetConfigurationResponse\x12.\n\x04meta\x18\x01 \x01(\x0b\x32 .blockjoy.api.ui_v1.ResponseMeta\x12>\n\x06params\x18\x02 \x03(\x0b\x32..blockjoy.api.ui_v1.UserConfigurationParameter2\xa5\x03\n\x0bUserService\x12P\n\x03Get\x12\".blockjoy.api.ui_v1.GetUserRequest\x1a#.blockjoy.api.ui_v1.GetUserResponse\"\x00\x12Y\n\x06\x43reate\x12%.blockjoy.api.ui_v1.CreateUserRequest\x1a&.blockjoy.api.ui_v1.CreateUserResponse\"\x00\x12x\n\x13UpsertConfiguration\x12..blockjoy.api.ui_v1.UpsertConfigurationRequest\x1a/.blockjoy.api.ui_v1.UpsertConfigurationResponse\"\x00\x12o\n\x10GetConfiguration\x12+.blockjoy.api.ui_v1.GetConfigurationRequest\x1a,.blockjoy.api.ui_v1.GetConfigurationResponse\"\x00\x62\x06proto3')

_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, globals())
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, 'user_service_pb2', globals())
if _descriptor._USE_C_DESCRIPTORS == False:

  DESCRIPTOR._options = None
  _GETUSERREQUEST._serialized_start=56
  _GETUSERREQUEST._serialized_end=119
  _GETUSERRESPONSE._serialized_start=121
  _GETUSERRESPONSE._serialized_end=226
  _CREATEUSERREQUEST._serialized_start=228
  _CREATEUSERREQUEST._serialized_end=334
  _CREATEUSERRESPONSE._serialized_start=336
  _CREATEUSERRESPONSE._serialized_end=404
  _UPSERTCONFIGURATIONREQUEST._serialized_start=407
  _UPSERTCONFIGURATIONREQUEST._serialized_end=546
  _UPSERTCONFIGURATIONRESPONSE._serialized_start=548
  _UPSERTCONFIGURATIONRESPONSE._serialized_end=625
  _GETCONFIGURATIONREQUEST._serialized_start=627
  _GETCONFIGURATIONREQUEST._serialized_end=699
  _GETCONFIGURATIONRESPONSE._serialized_start=702
  _GETCONFIGURATIONRESPONSE._serialized_end=840
  _USERSERVICE._serialized_start=843
  _USERSERVICE._serialized_end=1264
# @@protoc_insertion_point(module_scope)
