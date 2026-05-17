import { create } from 'zustand';

type ThemeMode = 'light' | 'dark';

type AppStore = {
  apiBaseUrl: string;
  mobileNavOpen: boolean;
  theme: ThemeMode;
  setApiBaseUrl: (apiBaseUrl: string) => void;
  setMobileNavOpen: (mobileNavOpen: boolean) => void;
  toggleTheme: () => void;
};

export const useAppStore = create<AppStore>((set) => ({
  apiBaseUrl: '',
  mobileNavOpen: false,
  theme: 'light',
  setApiBaseUrl: (apiBaseUrl) => set({ apiBaseUrl }),
  setMobileNavOpen: (mobileNavOpen) => set({ mobileNavOpen }),
  toggleTheme: () =>
    set((state) => ({
      theme: state.theme === 'light' ? 'dark' : 'light',
    })),
}));
