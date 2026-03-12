// FILE: W3DAssetManagerExposed.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  W3DAssetManagerExposed.h                                      */
/* Created:    John K. McDonald, Jr., 4/27/2002                              */
/* Desc:       A hack to get around our build structure.                     */
/* Revision History:                                                         */
/*		4/27/2002 : Initial creation                                           */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_W3DASSETMANAGEREXPOSED_
#define _H_W3DASSETMANAGEREXPOSED_

// INCLUDES ///////////////////////////////////////////////////////////////////
// DEFINES ////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
// FORWARD DECLARATIONS ///////////////////////////////////////////////////////

// This function is here because the particle editor needs to be able to force
// the asset manager to release all his textures and then reload them on demand.
// Unfortunately, the asset manager can't be called directly from the gamelogic,
// so this function is here. It should only be called by the particle editor,
// @todo Remove this function when we are no longer editing particles.
void ReloadAllTextures(void);

#endif /* _H_W3DASSETMANAGEREXPOSED_ */
