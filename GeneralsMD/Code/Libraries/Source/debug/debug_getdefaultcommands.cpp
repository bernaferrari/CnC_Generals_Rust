/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/debug_getdefaultcommands.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// DebugGetDefaultCommands function
//////////////////////////////////////////////////////////////////////////////
#include "_pch.h"

// this function has its own file so that it can be 'overridden'
// by another program using the Debug module
const char *DebugGetDefaultCommands(void)
{
  return "!debug.io flat add";
}
