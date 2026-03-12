
#pragma once

#include "Lib/BaseType.h"

class DebugWindowDialog;

class CSwitchesDialog : public CDialog
{
	public:
		enum {IDD = IDD_PSEd_EditSwitchesDialog};
		CSwitchesDialog(UINT nIDTemplate = CSwitchesDialog::IDD, CWnd* pParentWnd = NULL);

		void InitPanel( void );

		// if true, updates the UI from the Particle System. 
		// if false, updates the Particle System from the UI
		void performUpdate( IN Bool toUI );	
		DebugWindowDialog* GetDWDParent(void) { return (DebugWindowDialog*) GetParent(); }

	protected:
		afx_msg void OnParticleSystemEdit();
		DECLARE_MESSAGE_MAP()
};

