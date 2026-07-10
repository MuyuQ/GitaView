#import <Foundation/Foundation.h>
#import <objc/runtime.h>
#import <objc/message.h>

/// 供 Rust 调用的桥接函数，通知 WidgetKit 刷新所有时间线
/// 通过 objc runtime 动态加载 WidgetKit，避免编译时硬依赖
void reload_widget_timelines(void) {
    // 动态获取 WidgetCenter 类（macOS 11+ 才存在）
    Class widgetCenterClass = objc_getClass("WidgetCenter");
    if (!widgetCenterClass) {
        // WidgetKit 不可用，静默返回
        return;
    }

    // 获取 sharedCenter 单例
    SEL sharedSel = sel_registerName("sharedCenter");
    id (*sharedCenterIMP)(id, SEL) = (id(*)(id, SEL))objc_msgSend;
    id center = sharedCenterIMP((id)widgetCenterClass, sharedSel);

    // 调用 reloadAllTimelines
    SEL reloadSel = sel_registerName("reloadAllTimelines");
    void (*reloadIMP)(id, SEL) = (void(*)(id, SEL))objc_msgSend;
    reloadIMP(center, reloadSel);
}
