// FILE: EmissionTypePanels.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  EmissionTypePanels.h                                          */
/* Created:    John K. McDonald, Jr., 3/21/2002                              */
/* Desc:       Emission panels are pretty similar, they all go here.         */
/* Revision History:                                                         */
/*		3/21/2002 : Initial creation                                           */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_EMISSIONTYPEPANELS_
#define _H_EMISSIONTYPEPANELS_

// INCLUDES ///////////////////////////////////////////////////////////////////
#include "resource.h"
#include "ISwapablePanel.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// FORWARD DECLARATIONS ///////////////////////////////////////////////////////

// EmissionPanelPoint //////////////////////////////////////////////////////////
class EmissionPanelPoint : public ISwapablePanel
{
	public:
		enum {IDD = IDD_PSEd_EmissionPanelPoint};
		virtual DWORD GetIDD( void ) { return IDD; }
		EmissionPanelPoint(UINT nIDTemplate = EmissionPanelPoint::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	
	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

// EmissionPanelLine //////////////////////////////////////////////////////////
class EmissionPanelLine : public ISwapablePanel
{
	public:
		enum {IDD = IDD_PSEd_EmissionPanelLine};
		virtual DWORD GetIDD( void ) { return IDD; }
		EmissionPanelLine(UINT nIDTemplate = EmissionPanelLine::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	
	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

// EmissionPanelBox ///////////////////////////////////////////////////////////
class EmissionPanelBox : public ISwapablePanel
{
	public:
		enum {IDD = IDD_PSEd_EmissionPanelBox};
		virtual DWORD GetIDD( void ) { return IDD; }
		EmissionPanelBox(UINT nIDTemplate = EmissionPanelBox::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	
	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

// EmissionPanelSphere ////////////////////////////////////////////////////////
class EmissionPanelSphere : public ISwapablePanel
{
	public:
		enum {IDD = IDD_PSEd_EmissionPanelSphere};
		virtual DWORD GetIDD( void ) { return IDD; }
		EmissionPanelSphere(UINT nIDTemplate = EmissionPanelSphere::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	
	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

// EmissionPanelCylinder //////////////////////////////////////////////////////
class EmissionPanelCylinder : public ISwapablePanel
{
	public:
		enum {IDD = IDD_PSEd_EmissionPanelCylinder};
		virtual DWORD GetIDD( void ) { return IDD; }
		EmissionPanelCylinder(UINT nIDTemplate = EmissionPanelCylinder::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	
	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

#endif /* _H_EMISSIONTYPEPANELS_ */
