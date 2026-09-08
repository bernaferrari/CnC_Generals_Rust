/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/_pch.h $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Precompiled header (module internal)
//////////////////////////////////////////////////////////////////////////////
#ifdef _MSC_VER
#  pragma once
#endif
#ifndef _PCH_H // Include guard
#define _PCH_H

#include "debug.h"

// we need windows.h at too many places...
#define STRICT
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include "internal.h"
#include "internal_io.h"
#include "internal_except.h"

#endif // _PCH_H
