#ifndef SHDLIB_H
#define SHDLIB_H

#ifdef USE_WWSHADE

extern void SHD_Init();
extern void SHD_Shutdown();
extern void SHD_Init_Shaders();
extern void SHD_Shutdown_Shaders();
extern void SHD_Flush();
extern void SHD_Register_Loader();

#define SHD_INIT					SHD_Init()
#define SHD_SHUTDOWN				SHD_Shutdown()
#define SHD_INIT_SHADERS		SHD_Init_Shaders()
#define SHD_SHUTDOWN_SHADERS	SHD_Shutdown_Shaders()
#define SHD_FLUSH					SHD_Flush()
#define SHD_REG_LOADER			SHD_Register_Loader()

#else // USE_WWSHADE

#define SHD_INIT					
#define SHD_SHUTDOWN				
#define SHD_INIT_SHADERS		
#define SHD_SHUTDOWN_SHADERS	
#define SHD_FLUSH					
#define SHD_REG_LOADER			

#endif // USE_WWSHADE


#endif // SHDLIB_H