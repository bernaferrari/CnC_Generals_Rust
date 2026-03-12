#ifndef __MESH_DEFORM_PANEL_H
#define __MESH_DEFORM_PANEL_H

#include <Max.h>
#include "Resource.H"

// Forward declarations
class MeshDeformClass;

///////////////////////////////////////////////////////////////////////////
//
//	MeshDeformPanelClass
//
///////////////////////////////////////////////////////////////////////////
class MeshDeformPanelClass
{
	public:
		
		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		MeshDeformPanelClass (HWND hwnd)
			:	m_hWnd (hwnd),
				m_pColorSwatch (NULL),
				m_pMaxSetsSpin (NULL),
				m_pMeshDeformer (NULL),
				m_pLockSetsButton (NULL),
				m_pMaxSetsEdit (NULL)				{ }
		virtual ~MeshDeformPanelClass (void)	{ }

		//////////////////////////////////////////////////////////////////////
		// Public methods
		//////////////////////////////////////////////////////////////////////		

		// Inline accessors
		IColorSwatch *				Get_Color_Swatch (void) const			{ return m_pColorSwatch; }
		COLORREF						Get_Vertex_Color (void) const			{ return m_pColorSwatch->GetColor (); }
		void							Set_Vertex_Color (COLORREF color)	{ m_pColorSwatch->SetColor (color); }
		void							Set_Deformer (MeshDeformClass *obj);
		BOOL							Is_Edit_Mode (void) const				{ return (::SendDlgItemMessage (m_hWnd, IDC_STATE_SLIDER, TBM_GETPOS, 0, 0L) > 0); }
		BOOL							Are_Sets_Tied (void) const				{ return m_pLockSetsButton->IsChecked (); }
		int							Get_Current_Set (void) const			{ return ::SendDlgItemMessage (m_hWnd, IDC_CURRENT_SET_SLIDER, TBM_GETPOS, 0, 0L); }
		void							Set_Current_Set (int set, bool notify = false);
		void							Set_Max_Sets (int max, bool notify = false);
		void							Set_Current_State (float state);
		void							Set_Auto_Apply_Check (bool onoff)	{ ::SendDlgItemMessage (m_hWnd, IDC_MANUALAPPLY, BM_SETCHECK, (WPARAM)(!onoff), 0L); }
		bool							Get_Auto_Apply_Check (void) const	{ return ::SendDlgItemMessage (m_hWnd, IDC_MANUALAPPLY, BM_GETCHECK, 0, 0L) == 0; }

		// Update methods
		void							Update_Vertex_Color (void);

		//////////////////////////////////////////////////////////////////////
		// Static methods
		//////////////////////////////////////////////////////////////////////
		static BOOL WINAPI				Message_Proc (HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam);
		static MeshDeformPanelClass *	Get_Object (HWND hwnd);

	protected:

		//////////////////////////////////////////////////////////////////////
		// Protected methods
		//////////////////////////////////////////////////////////////////////		
		BOOL							On_Message (UINT message, WPARAM wparam, LPARAM lparam);
		void							On_Command (WPARAM wparam, LPARAM lparam);
	
	private:

		//////////////////////////////////////////////////////////////////////
		//	Private member data
		//////////////////////////////////////////////////////////////////////
		HWND							m_hWnd;
		IColorSwatch *				m_pColorSwatch;
		ICustEdit *					m_pMaxSetsEdit;
		ISpinnerControl *			m_pMaxSetsSpin;
		ICustButton *				m_pLockSetsButton;
		MeshDeformClass *			m_pMeshDeformer;
};


#endif //__MESH_DEFORM_PANEL_H
