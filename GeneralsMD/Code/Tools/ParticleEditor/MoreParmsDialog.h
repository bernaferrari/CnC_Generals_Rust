// FILE: MoreParmsDialog.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  MoreParmsDialog.h                                                      */
/* Created:    John K. McDonald, Jr., 3/23/2002                               */
/* Desc:       // @todo                                                      */
/* Revision History:                                                         */
/*		3/23/2002 : Initial creation                                          */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_MOREPARMSDIALOG_
#define _H_MOREPARMSDIALOG_

// INCLUDES ///////////////////////////////////////////////////////////////////
#include "resource.h"
#include "Lib/BaseType.h"

// DEFINES ////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
// FORWARD DECLARATIONS ///////////////////////////////////////////////////////

class MoreParmsDialog : public CDialog
{
	public:
		enum { IDD = IDD_PSEd_EditMoreParms };
		MoreParmsDialog(UINT nIDTemplate = MoreParmsDialog::IDD, CWnd* pParentWnd = NULL);
		virtual ~MoreParmsDialog();

		void InitPanel( void );
	
		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );

	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

#endif /* _H_MOREPARMSDIALOG_ */
