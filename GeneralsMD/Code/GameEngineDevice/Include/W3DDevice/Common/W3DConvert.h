//
// Project:    RTS3
//
// File name:  W3DConvert.h
//
// Created:    Colin Day, April 2001
//
//=============================================================================

#pragma once

#ifndef __W3DCONVERT_H_
#define __W3DCONVERT_H_

//=============================================================================
//           Includes                                                      
//=============================================================================
#include "Lib/BaseType.h"

//=============================================================================
//           Forward References
//=============================================================================
extern void W3DLogicalScreenToPixelScreen( Real logX, Real logY,
																					 Int *screenX, Int *screenY,
																					 Int screenWidth, Int screenHeight );
extern void PixelScreenToW3DLogicalScreen( Int screenX, Int screenY,
																					 Real *logX, Real *logY,
																					 Int screenWidth, Int screenHeight );

//=============================================================================
//           Type Defines
//=============================================================================


#endif // _W3DCONVERT_H_

