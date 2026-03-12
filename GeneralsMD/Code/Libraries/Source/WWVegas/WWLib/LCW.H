/*********************************************************************************************** 
 ***              C O N F I D E N T I A L  ---  W E S T W O O D  S T U D I O S               *** 
 *********************************************************************************************** 
 *                                                                                             * 
 *                 Project Name : Command & Conquer                                            * 
 *                                                                                             * 
 *                     $Archive:: /G/wwlib/lcw.h                                              $* 
 *                                                                                             * 
 *                      $Author:: Neal_k                                                      $*
 *                                                                                             * 
 *                     $Modtime:: 10/04/99 10:25a                                             $*
 *                                                                                             * 
 *                    $Revision:: 3                                                           $*
 *                                                                                             *
 *---------------------------------------------------------------------------------------------* 
 * Functions:                                                                                  * 
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */
#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef LCW_H
#define LCW_H

#ifdef _UNIX
#include "osdep.h"
#endif

int LCW_Uncomp(void const * source, void * dest, unsigned long length=0);

#ifdef _MSC_VER
int LCW_Comp(void const * source, void * dest, int length);
#else
extern "C" {
int __cdecl LCW_Comp(void const * source, void * dest, int length);
}
#endif

#endif
