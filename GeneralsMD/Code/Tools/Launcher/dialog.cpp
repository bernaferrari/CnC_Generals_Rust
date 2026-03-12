//
// Create the dialog used during the patching process.
//
#include"winblows.h"
#include"resource.h"
#include"loadbmp.h"
#include<commctrl.h>

HWND PatchDialog;
BOOL CALLBACK Patch_Window_Proc(HWND hwnd, UINT iMsg, WPARAM wParam, LPARAM lParam);

HWND Create_Patch_Dialog(void)
{
  PatchDialog=CreateDialog(Global_instance, MAKEINTRESOURCE(IDD_PATCHPROGRESS),
    NULL, (DLGPROC)Patch_Window_Proc);

  ShowWindow(PatchDialog, SW_NORMAL);
  SetForegroundWindow(PatchDialog);
  return(PatchDialog);
}

BOOL CALLBACK Patch_Window_Proc(HWND hwnd, UINT iMsg, WPARAM wParam, LPARAM lParam)
{

  static LoadBmp bmpLoader;

  switch(iMsg) {
    case WM_INITDIALOG:
      // progress bar
      SendMessage(GetDlgItem(hwnd,IDC_PROGRESS2),PBM_SETRANGE,
        0,MAKELPARAM(0,100));
      SendMessage(GetDlgItem(hwnd,IDC_PROGRESS2),PBM_SETPOS,0,0);
      SendMessage(GetDlgItem(hwnd,IDC_PROGRESS2),PBM_SETSTEP,10,0);

      bmpLoader.init("launcher.bmp",GetDlgItem(hwnd,IDC_SPLASH));
      return(TRUE);   // True means windows handles focus issues
    break;
    case WM_PAINT:
      bmpLoader.drawBmp();
    break;
    case WM_COMMAND:
      /* May want to add cancel later
      switch(wParam) {
        case IDCANCEL:
        {
          // do some stuff
          return(TRUE);
        }
        default:
        break;
      }
      default:
      *************/
    break;
    case WM_CLOSE:
      DestroyWindow(hwnd);
      PostQuitMessage(0);
      exit(0);
    break;
  }
  return(FALSE);
}