#ifndef SCENESETUPDLG_H
#define SCENESETUPDLG_H

// SceneSetupDlg.h : header file
//

#include "dllmain.h"
#include "Resource.h"


class Interface;

/////////////////////////////////////////////////////////////////////////////
// SceneSetupDlg dialog

class SceneSetupDlg
{
public:

	// Construction
	SceneSetupDlg(Interface *max_interface);

	// Methods
	int DoModal (void);

	// DialogProc
	BOOL CALLBACK DialogProc (HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam);

	// Dialog data associated with GUI components.
	enum	{ IDD = IDD_SCENE_SETUP };
	int	m_DamageCount;
	float	m_DamageOffset;
	int	m_LodCount;
	float	m_LodOffset;
	int	m_LodProc;
	int	m_DamageProc;

	// Dialog Data
	HWND			m_hWnd;

protected:

	// Message Handlers
	void OnInitDialog (void);
	BOOL OnOK (void);		// TRUE if ok to close dialog

	// Protected Methods
	void  SetEditInt   (int control_id, int value);
	void  SetEditFloat (int control_id, float value);
	int   GetEditInt   (int control_id);
	float GetEditFloat (int control_id);
	bool  ValidateEditFloat (int control_id);

	// Protected Data
	Interface	*m_MaxInterface;
};

#endif
