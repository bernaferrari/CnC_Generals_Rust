// FILE: ISwapablePanel.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  ISwapablePanel.h                                              */
/* Created:    John K. McDonald, Jr., 3/21/2002                              */
/* Desc:       Swapable panels derive from this so that we can easily call   */
/*						 the update function                                           */
/* Revision History:                                                         */
/*		3/21/2002 : Initial creation                                           */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_ISWAPABLEPANEL_
#define _H_ISWAPABLEPANEL_

#include "Lib/BaseType.h"

// INCLUDES ///////////////////////////////////////////////////////////////////
// DEFINES ////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
// FORWARD DECLARATIONS ///////////////////////////////////////////////////////

interface ISwapablePanel : public CDialog
{
	ISwapablePanel(UINT nIDTemplate = 0, CWnd* pParentWnd = NULL) : CDialog(nIDTemplate, pParentWnd) {}
	virtual DWORD GetIDD( void ) = 0;
	virtual void performUpdate( IN Bool toUI ) = 0;
	virtual void InitPanel( void ) = 0;
};

#endif /* _H_ISWAPABLEPANEL_ */
