#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __FORMCLASS_H
#define __FORMCLASS_H

#include <Max.h>


class FormClass : public ParamDlg
{
	public:
		FormClass (void) 
			: m_hWnd (NULL) {}
		~FormClass (void) {}

		HWND						Create_Form (HWND parent_wnd, UINT template_id);
		void						Show (bool show_flag = true) { ::ShowWindow (m_hWnd, show_flag ? SW_SHOW : SW_HIDE); }
		virtual BOOL			Dialog_Proc (HWND dlg_wnd, UINT message, WPARAM wparam, LPARAM lparam) = 0;
		HWND						Get_Hwnd(void) { return m_hWnd; }
		virtual void			Invalidate(void) { InvalidateRect(m_hWnd,NULL,0); }

	protected:
		
		BOOL						ExecuteDlgInit(LPVOID lpResource);
		BOOL						ExecuteDlgInit(LPCTSTR lpszResourceName);

		static BOOL	WINAPI	fnFormProc (HWND dlg_wnd, UINT message, WPARAM wparam,  LPARAM lparam);

		HWND						m_hWnd;
		RECT						m_FormRect;
};

#endif //__FORMCLASS_H
