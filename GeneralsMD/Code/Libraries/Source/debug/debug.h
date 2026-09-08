/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/debug.h $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Debugging module
//////////////////////////////////////////////////////////////////////////////
#ifdef _MSC_VER
#  pragma once
#endif
#ifndef DEBUG_H // Include guard
#define DEBUG_H

/** 
  \page lib_var Library variants

  Generally speaking there are four different library variants:
  - Internal: all asserts/checks/logs, full optimizations (_INTERNAL macro defined)
  - %Debug: all asserts/checks/logs, no optimizations (_DEBUG macro defined)
  - Profile: all asserts/checks/logs, full optimizations, profiling active (_PROFILE macro defined)
  - Release: no asserts/checks/logs, full optimizations

  These variants will be broken down into separate features which
  can be queried for by #ifdef(HAS_FEATURE):

  <table><tr>
    <td><b>Variant</b></td>
    <td><b>HAS_ASSERTS</b></td>
    <td><b>HAS_LOGS</b></td>
    <td><b>HAS_OPT</b></td>
    <td><b>HAS_PROFILE</b></td>
  </tr><tr>
    <td>Internal</td>
    <td><center>Y</center></td>
    <td><center>Y</center></td>
    <td><center>Y</center></td>
    <td><center></center></td>
  </tr><tr>
    <td>%Debug</td>
    <td><center>Y</center></td>
    <td><center>Y</center></td>
    <td><center></center></td>
    <td><center></center></td>
  </tr><tr>
    <td>Profile</td>
    <td><center>Y</center></td>
    <td><center>Y</center></td>
    <td><center>Y</center></td>
    <td><center>Y</center></td>
  </tr><tr>
    <td>Release</td>
    <td><center></center></td>
    <td><center></center></td>
    <td><center>Y</center></td>
    <td><center></center></td>
  </tr></table>

  Library files have a suffix appended that depends on the
  library variant:
  - Internal: XXXInternal.lib
  - %Debug: XXXDebug.lib
  - Profile: XXXProfile.lib
  - Release: XXX.lib
*/

#if defined(_DEBUG) && defined(_INTERNAL)
	#error "Only either _DEBUG or _INTERNAL should ever be defined"
#endif

// Define which libraries to use. 
#if defined(_INTERNAL)
#  pragma comment (lib,"debuginternal.lib")
#  define HAS_ASSERTS
#  define HAS_LOGS
#  define HAS_OPT
#elif defined(_DEBUG)
#  pragma comment (lib,"debugdebug.lib")
#  define HAS_ASSERTS
#  define HAS_LOGS
#elif defined(_PROFILE)
#  pragma comment (lib,"debugprofile.lib")
#  define HAS_ASSERTS
#  define HAS_LOGS
#  define HAS_OPT
#  define HAS_PROFILE
#else
#  pragma comment (lib,"debug.lib")
#  define HAS_OPT
#endif

// include all our public header files (use double quotes here)
#include "debug_doc.h"
#include "debug_macro.h"
#include "debug_io.h"
#include "debug_cmd.h"
#include "debug_stack.h"
#include "debug_debug.h"

#endif // DEBUG_H
