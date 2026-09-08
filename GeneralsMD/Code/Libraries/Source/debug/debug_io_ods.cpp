/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/debug_io_ods.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Debug I/O class ods (OutputDebugString, for use in debugger)
//////////////////////////////////////////////////////////////////////////////
#include "_pch.h"
#include <new>      // needed for placement new prototype

void DebugIOOds::Write(StringType type, const char *src, const char *str)
{
  if (type!=StringType::StructuredCmdReply&&str)
    OutputDebugString(str);
}

DebugIOInterface *DebugIOOds::Create(void)
{
  return new (DebugAllocMemory(sizeof(DebugIOOds))) DebugIOOds();
}

void DebugIOOds::Delete(void)
{
  this->~DebugIOOds();
  DebugFreeMemory(this);
}
