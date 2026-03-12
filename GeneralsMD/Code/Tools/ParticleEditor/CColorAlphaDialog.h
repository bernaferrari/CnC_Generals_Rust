
#pragma once

#include "CButtonShowColor.h"
#include "GameClient/ParticleSys.h"

class CColorAlphaDialog : public CDialog
{
	protected:	
		DWORD m_customColors[16];
		CButtonShowColor m_colorButton[MAX_KEYFRAMES];

		void onColorPress( Int colorPressed );
	public:
		enum {IDD = IDD_PSEd_EditColorAndAlpha};
		CColorAlphaDialog(UINT nIDTemplate = CColorAlphaDialog::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	


	protected:
		virtual BOOL OnInitDialog();

		afx_msg void OnColor1();
		afx_msg void OnColor2();
		afx_msg void OnColor3();
		afx_msg void OnColor4();
		afx_msg void OnColor5();
		afx_msg void OnColor6();
		afx_msg void OnColor7();
		afx_msg void OnColor8();
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

